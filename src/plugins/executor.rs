//! Script execution engine for plugin handlers and hooks.
//!
//! Scripts receive JSON on stdin, emit JSON on stdout, and get context via
//! environment variables.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

/// Default execution timeout.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Context passed to a script execution.
#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    /// Extra environment variables for the script.
    pub env_vars: HashMap<String, String>,
    /// Optional stdin data (e.g. JSON request body).
    pub stdin: Option<Vec<u8>>,
}

/// Result of executing a script.
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
    pub timed_out: bool,
}

/// Async script executor with configurable timeout.
#[derive(Debug, Clone)]
pub struct ScriptExecutor {
    timeout: Duration,
}

impl ScriptExecutor {
    /// Create a new executor with the given timeout.
    #[allow(dead_code)]
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Create a new executor with the default timeout.
    pub fn with_default_timeout() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Execute a script at the given path with the provided context.
    pub async fn execute(
        &self,
        script_path: &Path,
        ctx: &ExecutionContext,
    ) -> Result<ExecutionResult> {
        let (program, args) = resolve_interpreter(script_path);

        let mut cmd = Command::new(&program);
        for arg in &args {
            cmd.arg(arg);
        }

        // Set working directory to the script's parent.
        if let Some(parent) = script_path.parent() {
            cmd.current_dir(parent);
        }

        // Inject environment variables.
        for (k, v) in &ctx.env_vars {
            cmd.env(k, v);
        }

        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            anyhow::anyhow!("failed to spawn script '{}': {}", script_path.display(), e)
        })?;

        // Write stdin if provided, then close it.
        if let Some(data) = &ctx.stdin {
            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                let _ = stdin.write_all(data).await;
                drop(stdin);
            }
        } else {
            drop(child.stdin.take());
        }

        // Take stdout/stderr handles for manual reading.
        let mut stdout_handle = child.stdout.take();
        let mut stderr_handle = child.stderr.take();

        // Read stdout and stderr concurrently with waiting.
        use tokio::io::AsyncReadExt;
        let read_all = async {
            let mut stdout_buf = Vec::new();
            let mut stderr_buf = Vec::new();
            if let Some(ref mut h) = stdout_handle {
                let _ = h.read_to_end(&mut stdout_buf).await;
            }
            if let Some(ref mut h) = stderr_handle {
                let _ = h.read_to_end(&mut stderr_buf).await;
            }
            let status = child.wait().await;
            (stdout_buf, stderr_buf, status)
        };

        match tokio::time::timeout(self.timeout, read_all).await {
            Ok((stdout_buf, stderr_buf, Ok(status))) => Ok(ExecutionResult {
                stdout: stdout_buf,
                stderr: stderr_buf,
                exit_code: status.code().unwrap_or(-1),
                timed_out: false,
            }),
            Ok((_, _, Err(e))) => Err(anyhow::anyhow!(
                "script '{}' I/O error: {}",
                script_path.display(),
                e
            )),
            Err(_) => {
                // Timeout — kill the child process.
                // child was moved into the future, so we can't kill it here
                // directly. The drop of the future will clean up the child.
                Ok(ExecutionResult {
                    stdout: Vec::new(),
                    stderr: format!(
                        "script '{}' timed out after {:?}",
                        script_path.display(),
                        self.timeout
                    )
                    .into_bytes(),
                    exit_code: -1,
                    timed_out: true,
                })
            }
        }
    }
}

/// Resolve the interpreter and arguments for a script path.
///
/// On Unix, scripts use shebang lines — we just call the script directly.
/// On Windows, we map file extensions to interpreters.
fn resolve_interpreter(script_path: &Path) -> (String, Vec<String>) {
    #[cfg(unix)]
    {
        (script_path.to_string_lossy().to_string(), vec![])
    }
    #[cfg(windows)]
    {
        let ext = script_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        match ext.to_lowercase().as_str() {
            "sh" | "bash" => (
                "bash".to_string(),
                vec![script_path.to_string_lossy().to_string()],
            ),
            "py" | "python" => (
                "python".to_string(),
                vec![script_path.to_string_lossy().to_string()],
            ),
            "js" | "mjs" => (
                "node".to_string(),
                vec![script_path.to_string_lossy().to_string()],
            ),
            "ps1" => (
                "powershell".to_string(),
                vec![
                    "-ExecutionPolicy".to_string(),
                    "Bypass".to_string(),
                    "-File".to_string(),
                    script_path.to_string_lossy().to_string(),
                ],
            ),
            "bat" | "cmd" => (
                "cmd".to_string(),
                vec!["/c".to_string(), script_path.to_string_lossy().to_string()],
            ),
            _ => (script_path.to_string_lossy().to_string(), vec![]),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn tmp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ac-executor-test-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Write a platform-appropriate script.
    ///
    /// On Unix: writes a `.sh` file with shebang and +x permission.
    /// On Windows: writes a `.cmd` file with batch syntax.
    fn write_script(dir: &Path, base_name: &str, unix_content: &str) -> PathBuf {
        #[cfg(unix)]
        {
            let path = dir.join(format!("{}.sh", base_name));
            fs::write(&path, unix_content).unwrap();
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            path
        }
        #[cfg(windows)]
        {
            // Map common shell scripts to batch equivalents.
            let _ = unix_content; // used only on Unix
            let bat = match base_name {
                "hello" => "@echo off\necho hello world\n",
                "echo_stdin" => {
                    // Windows batch: read stdin via `findstr` (pass-through)
                    "@echo off\nfindstr \"^\" \n"
                }
                "env" => "@echo off\necho %MY_VAR%\n",
                "fail" => "@echo off\nexit /b 42\n",
                "stderr" => "@echo off\necho error msg >&2\nexit /b 1\n",
                "slow" => "@echo off\nping -n 30 127.0.0.1 >nul\n",
                _ => "@echo off\n",
            };
            let path = dir.join(format!("{}.cmd", base_name));
            fs::write(&path, bat).unwrap();
            path
        }
    }

    #[tokio::test]
    async fn execute_simple_script() {
        let dir = tmp_dir("simple");
        let script = write_script(&dir, "hello", "#!/bin/sh\necho hello world\n");

        let executor = ScriptExecutor::with_default_timeout();
        let ctx = ExecutionContext::default();
        let result = executor.execute(&script, &ctx).await.unwrap();

        assert_eq!(result.exit_code, 0);
        assert!(!result.timed_out);
        assert_eq!(
            String::from_utf8_lossy(&result.stdout).trim(),
            "hello world"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    #[cfg(unix)] // stdin piping to batch `findstr` is unreliable; test on Unix only.
    async fn execute_script_with_stdin() {
        let dir = tmp_dir("stdin");
        let script = write_script(&dir, "echo_stdin", "#!/bin/sh\ncat\n");

        let executor = ScriptExecutor::with_default_timeout();
        let ctx = ExecutionContext {
            stdin: Some(b"{\"key\": \"value\"}".to_vec()),
            ..Default::default()
        };
        let result = executor.execute(&script, &ctx).await.unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(
            String::from_utf8_lossy(&result.stdout),
            "{\"key\": \"value\"}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn execute_script_with_env_vars() {
        let dir = tmp_dir("env");
        let script = write_script(&dir, "env", "#!/bin/sh\necho $MY_VAR\n");

        let executor = ScriptExecutor::with_default_timeout();
        let mut env = HashMap::new();
        env.insert("MY_VAR".to_string(), "test-value".to_string());
        let ctx = ExecutionContext {
            env_vars: env,
            ..Default::default()
        };
        let result = executor.execute(&script, &ctx).await.unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "test-value");

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn execute_script_nonzero_exit() {
        let dir = tmp_dir("nonzero");
        let script = write_script(&dir, "fail", "#!/bin/sh\nexit 42\n");

        let executor = ScriptExecutor::with_default_timeout();
        let ctx = ExecutionContext::default();
        let result = executor.execute(&script, &ctx).await.unwrap();

        assert_eq!(result.exit_code, 42);
        assert!(!result.timed_out);

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn execute_script_stderr() {
        let dir = tmp_dir("stderr");
        let script = write_script(&dir, "stderr", "#!/bin/sh\necho 'error msg' >&2\nexit 1\n");

        let executor = ScriptExecutor::with_default_timeout();
        let ctx = ExecutionContext::default();
        let result = executor.execute(&script, &ctx).await.unwrap();

        assert_eq!(result.exit_code, 1);
        assert!(String::from_utf8_lossy(&result.stderr).contains("error msg"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn execute_script_timeout() {
        let dir = tmp_dir("timeout");
        let script = write_script(&dir, "slow", "#!/bin/sh\nsleep 30\n");

        let executor = ScriptExecutor::new(Duration::from_millis(200));
        let ctx = ExecutionContext::default();
        let result = executor.execute(&script, &ctx).await.unwrap();

        assert!(result.timed_out);
        assert_eq!(result.exit_code, -1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn execute_nonexistent_script() {
        let executor = ScriptExecutor::with_default_timeout();
        let ctx = ExecutionContext::default();
        let result = executor
            .execute(Path::new("/nonexistent/script.sh"), &ctx)
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("failed to spawn"));
    }

    #[test]
    fn default_timeout_value() {
        let executor = ScriptExecutor::with_default_timeout();
        assert_eq!(executor.timeout, Duration::from_secs(5));
    }

    #[test]
    fn custom_timeout_value() {
        let executor = ScriptExecutor::new(Duration::from_secs(30));
        assert_eq!(executor.timeout, Duration::from_secs(30));
    }
}
