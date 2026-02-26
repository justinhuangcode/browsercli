//! End-to-end integration tests.
//!
//! These tests require a real Chromium browser and are gated behind the
//! `BROWSERCLI_E2E` environment variable. Run with:
//!
//! ```
//! BROWSERCLI_E2E=1 cargo test --test e2e_integration -- --ignored
//! ```

use std::process::Command;
use std::time::{Duration, Instant};

fn browsercli() -> Command {
    let bin = env!("CARGO_BIN_EXE_browsercli");
    Command::new(bin)
}

fn e2e_enabled() -> bool {
    std::env::var("BROWSERCLI_E2E").is_ok()
}

/// Poll a DOM text query until it returns the expected value or times out.
fn wait_for_dom_text(selector: &str, expected: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        let output = browsercli()
            .args(["--json", "dom", "query", selector, "--mode", "text"])
            .output()
            .unwrap();
        if output.status.success() {
            let json: serde_json::Value =
                serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap_or_default();
            if json["value"].as_str() == Some(expected) {
                return;
            }
        }
        if Instant::now() >= deadline {
            let stdout = String::from_utf8_lossy(&output.stdout);
            panic!(
                "timeout waiting for {} to contain {:?}, last output: {}",
                selector, expected, stdout
            );
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Full lifecycle: start -> write HTML -> goto -> dom query -> screenshot -> console -> network -> perf -> stop
#[test]
#[ignore]
fn e2e_full_lifecycle() {
    if !e2e_enabled() {
        eprintln!("skipping e2e: set BROWSERCLI_E2E=1 to enable");
        return;
    }

    let tmp = std::env::temp_dir().join(format!("browsercli-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();

    // Write a test HTML page.
    std::fs::write(
        tmp.join("index.html"),
        r#"<!DOCTYPE html>
<html>
<head><title>E2E Test Page</title></head>
<body>
  <h1 id="title">Hello E2E</h1>
  <button id="btn" onclick="document.getElementById('title').textContent='Clicked!'">Click Me</button>
  <script>console.log("page loaded");</script>
</body>
</html>"#,
    )
    .unwrap();

    // --- Start ---
    let output = browsercli()
        .args([
            "--json",
            "start",
            "--dir",
            tmp.to_str().unwrap(),
            "--headless",
        ])
        .output()
        .unwrap();
    let start_stdout = String::from_utf8_lossy(&output.stdout);
    let start_stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "start failed:\n  stdout: {}\n  stderr: {}",
        start_stdout,
        start_stderr
    );
    let start_json: serde_json::Value =
        serde_json::from_str(start_stdout.trim()).unwrap_or_else(|e| {
            panic!(
                "start did not produce valid JSON:\n  parse error: {}\n  stdout: {}\n  stderr: {}",
                e, start_stdout, start_stderr
            )
        });
    assert!(start_json["http_port"].as_u64().unwrap() > 0);

    // Ensure cleanup even if the test panics below.
    let _guard = DaemonGuard;

    // Give the browser a moment to load the page.
    std::thread::sleep(Duration::from_secs(2));

    // --- Status ---
    let output = browsercli().args(["--json", "status"]).output().unwrap();
    assert!(output.status.success(), "status failed");
    let status_json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert_eq!(status_json["running"], true);

    // --- Goto ---
    let output = browsercli().args(["--json", "goto", "/"]).output().unwrap();
    assert!(output.status.success(), "goto failed");

    // --- Dom query (with polling — CI can be slow) ---
    wait_for_dom_text("#title", "Hello E2E", Duration::from_secs(10));

    // --- Dom click ---
    let output = browsercli()
        .args(["--json", "dom", "click", "#btn"])
        .output()
        .unwrap();
    assert!(output.status.success(), "dom click failed");

    // Verify click result (with polling).
    wait_for_dom_text("#title", "Clicked!", Duration::from_secs(5));

    // --- Eval ---
    let output = browsercli()
        .args(["--json", "eval", "document.title"])
        .output()
        .unwrap();
    assert!(output.status.success(), "eval failed");
    let eval_json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert_eq!(eval_json["value"], "E2E Test Page");

    // --- Screenshot ---
    let screenshot_path = tmp.join("test-screenshot.png");
    let output = browsercli()
        .args([
            "--json",
            "screenshot",
            "--out",
            screenshot_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "screenshot failed");
    assert!(screenshot_path.exists(), "screenshot file should exist");
    assert!(
        std::fs::metadata(&screenshot_path).unwrap().len() > 100,
        "screenshot should not be empty"
    );

    // --- Console ---
    let output = browsercli().args(["--json", "console"]).output().unwrap();
    assert!(output.status.success(), "console failed");
    let console_json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert!(console_json["entries"].is_array());

    // --- Network ---
    let output = browsercli().args(["--json", "network"]).output().unwrap();
    assert!(output.status.success(), "network failed");
    let network_json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert!(network_json["entries"].is_array());

    // --- Perf ---
    let output = browsercli().args(["--json", "perf"]).output().unwrap();
    assert!(output.status.success(), "perf failed");
    let perf_json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert!(perf_json["dom_content_loaded_ms"].as_f64().unwrap() >= 0.0);

    // --- Stop ---
    let output = browsercli().args(["--json", "stop"]).output().unwrap();
    assert!(output.status.success(), "stop failed");
    let stop_json: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).unwrap();
    assert_eq!(stop_json["ok"], true);

    // Verify it's actually stopped.
    std::thread::sleep(Duration::from_millis(500));
    let output = browsercli().args(["status"]).output().unwrap();
    assert!(!output.status.success(), "should fail after stop");

    // Cleanup.
    let _ = std::fs::remove_dir_all(&tmp);
}

/// Helper to ensure the daemon is cleaned up even if the test panics.
struct DaemonGuard;

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        // Best-effort stop on test failure.
        let _ = browsercli().args(["stop"]).output();
    }
}
