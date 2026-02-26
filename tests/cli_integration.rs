use std::process::Command;

fn browsercli() -> Command {
    let bin = env!("CARGO_BIN_EXE_browsercli");
    Command::new(bin)
}

#[test]
fn help_output() {
    let output = browsercli().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("browser visual workspace"));
    assert!(stdout.contains("start"));
    assert!(stdout.contains("stop"));
    assert!(stdout.contains("goto"));
    assert!(stdout.contains("eval"));
    assert!(stdout.contains("dom"));
    assert!(stdout.contains("screenshot"));
    assert!(stdout.contains("console"));
    assert!(stdout.contains("network"));
    assert!(stdout.contains("perf"));
}

#[test]
fn version_output() {
    let output = browsercli().arg("--version").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("browsercli"));
}

#[test]
fn start_help() {
    let output = browsercli().args(["start", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--dir"));
    assert!(stdout.contains("--port"));
    assert!(stdout.contains("--headless"));
    assert!(stdout.contains("--no-app"));
    assert!(stdout.contains("--no-stealth"));
    assert!(stdout.contains("--window-size"));
    assert!(stdout.contains("--restart"));
}

#[test]
fn dom_help() {
    let output = browsercli().args(["dom", "--help"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("query"));
    assert!(stdout.contains("all"));
    assert!(stdout.contains("attr"));
    assert!(stdout.contains("click"));
    assert!(stdout.contains("type"));
    assert!(stdout.contains("wait"));
}

#[test]
fn status_without_daemon_fails() {
    let output = browsercli().arg("status").output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no running session") || stderr.contains("Error"),
        "expected session error, got: {}",
        stderr
    );
}

#[test]
fn stop_without_daemon_fails() {
    let output = browsercli().arg("stop").output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn goto_without_daemon_fails() {
    let output = browsercli().args(["goto", "/"]).output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn eval_without_daemon_fails() {
    let output = browsercli().args(["eval", "1+1"]).output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn invalid_command_fails() {
    let output = browsercli().arg("nonexistent").output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn json_flag_on_status() {
    let output = browsercli().args(["--json", "status"]).output().unwrap();
    // Should fail (no daemon) but output structured JSON error to stdout.
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(
        json.get("error").is_some(),
        "JSON error output should have 'error' field"
    );
    assert!(
        json.get("schema_version").is_some(),
        "JSON error output should have 'schema_version' field"
    );
}

#[test]
fn exit_code_nonzero_on_failure() {
    let output = browsercli().args(["status"]).output().unwrap();
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
}

#[test]
fn json_error_is_parseable() {
    for cmd in &["status", "stop"] {
        let output = browsercli().args(["--json", cmd]).output().unwrap();
        assert!(!output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
            panic!("--json {} should produce valid JSON on stdout: {}", cmd, e)
        });
        assert!(
            json["error"].is_string(),
            "--json {} error field should be a string",
            cmd
        );
    }
}

#[test]
fn missing_required_arg() {
    let output = browsercli().arg("goto").output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("required") || stderr.contains("Usage"));
}
