//! Lifecycle hook dispatch.
//!
//! Hooks are fire-and-forget: each handler runs in a `tokio::spawn` task,
//! errors are logged but never crash the daemon.

use std::collections::HashMap;

use crate::plugins::executor::{ExecutionContext, ScriptExecutor};
use crate::plugins::registry::PluginRegistry;

/// Events that can trigger plugin hooks.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum HookEvent {
    DaemonStart,
    DaemonStop,
    FileChange(String),
    Navigate(String),
    Console(serde_json::Value),
    Network(serde_json::Value),
}

impl HookEvent {
    /// The hook event name used in plugin.json manifests.
    pub fn name(&self) -> &str {
        match self {
            HookEvent::DaemonStart => "on_daemon_start",
            HookEvent::DaemonStop => "on_daemon_stop",
            HookEvent::FileChange(_) => "on_file_change",
            HookEvent::Navigate(_) => "on_navigate",
            HookEvent::Console(_) => "on_console",
            HookEvent::Network(_) => "on_network",
        }
    }

    /// Build event-specific environment variables.
    fn env_vars(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();
        match self {
            HookEvent::FileChange(path) => {
                env.insert("BROWSERCLI_FILE_PATH".to_string(), path.clone());
            }
            HookEvent::Navigate(url) => {
                env.insert("BROWSERCLI_URL".to_string(), url.clone());
            }
            _ => {}
        }
        env
    }

    /// Build optional stdin data for the event.
    fn stdin_data(&self) -> Option<Vec<u8>> {
        match self {
            HookEvent::Console(val) | HookEvent::Network(val) => serde_json::to_vec(val).ok(),
            _ => None,
        }
    }
}

/// Dispatch a hook event to all registered handlers.
///
/// Each handler runs concurrently in its own task. Errors are logged but
/// do not propagate — hooks are best-effort.
pub fn dispatch_hook(
    registry: &PluginRegistry,
    event: HookEvent,
    executor: &ScriptExecutor,
    base_env: &HashMap<String, String>,
) {
    let handlers = registry.get_hook_handlers(event.name());
    if handlers.is_empty() {
        return;
    }

    let event_env = event.env_vars();
    let stdin_data = event.stdin_data();

    for handler in handlers {
        let script_path = handler.script_path.clone();
        let plugin_name = handler.plugin_name.clone();
        let event_name = event.name().to_string();

        // Merge base env + event env + plugin name.
        let mut env = base_env.clone();
        for (k, v) in &event_env {
            env.insert(k.clone(), v.clone());
        }
        env.insert("BROWSERCLI_PLUGIN_NAME".to_string(), plugin_name.clone());

        let ctx = ExecutionContext {
            env_vars: env,
            stdin: stdin_data.clone(),
        };

        let executor = executor.clone();

        tokio::spawn(async move {
            match executor.execute(&script_path, &ctx).await {
                Ok(result) => {
                    if result.timed_out {
                        tracing::warn!(
                            plugin = %plugin_name,
                            event = %event_name,
                            "hook timed out"
                        );
                    } else if result.exit_code != 0 {
                        let stderr = String::from_utf8_lossy(&result.stderr);
                        tracing::warn!(
                            plugin = %plugin_name,
                            event = %event_name,
                            exit_code = result.exit_code,
                            stderr = %stderr.trim(),
                            "hook exited with error"
                        );
                    } else {
                        tracing::debug!(
                            plugin = %plugin_name,
                            event = %event_name,
                            "hook completed"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        plugin = %plugin_name,
                        event = %event_name,
                        error = %e,
                        "hook dispatch failed"
                    );
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_event_names() {
        assert_eq!(HookEvent::DaemonStart.name(), "on_daemon_start");
        assert_eq!(HookEvent::DaemonStop.name(), "on_daemon_stop");
        assert_eq!(
            HookEvent::FileChange("a.html".into()).name(),
            "on_file_change"
        );
        assert_eq!(HookEvent::Navigate("http://x".into()).name(), "on_navigate");
        assert_eq!(
            HookEvent::Console(serde_json::json!({})).name(),
            "on_console"
        );
        assert_eq!(
            HookEvent::Network(serde_json::json!({})).name(),
            "on_network"
        );
    }

    #[test]
    fn file_change_env_vars() {
        let event = HookEvent::FileChange("/tmp/test.html".to_string());
        let env = event.env_vars();
        assert_eq!(env.get("BROWSERCLI_FILE_PATH").unwrap(), "/tmp/test.html");
    }

    #[test]
    fn navigate_env_vars() {
        let event = HookEvent::Navigate("http://localhost:8080/".to_string());
        let env = event.env_vars();
        assert_eq!(env.get("BROWSERCLI_URL").unwrap(), "http://localhost:8080/");
    }

    #[test]
    fn daemon_start_no_extra_env() {
        let event = HookEvent::DaemonStart;
        assert!(event.env_vars().is_empty());
    }

    #[test]
    fn console_event_stdin() {
        let val = serde_json::json!({"level": "error", "text": "oops"});
        let event = HookEvent::Console(val.clone());
        let stdin = event.stdin_data().unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&stdin).unwrap();
        assert_eq!(parsed, val);
    }

    #[test]
    fn network_event_stdin() {
        let val = serde_json::json!({"url": "http://example.com", "status": 200});
        let event = HookEvent::Network(val.clone());
        let stdin = event.stdin_data().unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&stdin).unwrap();
        assert_eq!(parsed, val);
    }

    #[test]
    fn daemon_start_no_stdin() {
        assert!(HookEvent::DaemonStart.stdin_data().is_none());
    }

    #[test]
    fn file_change_no_stdin() {
        assert!(HookEvent::FileChange("x".into()).stdin_data().is_none());
    }

    #[test]
    fn navigate_no_stdin() {
        assert!(HookEvent::Navigate("x".into()).stdin_data().is_none());
    }
}
