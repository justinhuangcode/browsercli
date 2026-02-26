use std::path::Path;
use std::process::Command;

/// Search for a Chromium-based browser binary on the current platform.
///
/// Checks `CHROME_BIN` environment variable first, then searches PATH
/// and well-known install locations.
pub fn find_chromium_binary() -> Option<String> {
    // Respect CHROME_BIN env var (useful for CI and non-standard installs).
    if let Ok(bin) = std::env::var("CHROME_BIN") {
        if !bin.is_empty() && Path::new(&bin).exists() {
            return Some(bin);
        }
    }

    // Check PATH-based binaries first (works on all platforms).
    for name in &[
        "chromium",
        "chromium-browser",
        "google-chrome",
        "google-chrome-stable",
        "chrome",
    ] {
        #[cfg(unix)]
        let which_cmd = "which";
        #[cfg(windows)]
        let which_cmd = "where";
        if let Ok(output) = Command::new(which_cmd).arg(name).output() {
            if output.status.success() {
                let p = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !p.is_empty() {
                    return Some(p);
                }
            }
        }
    }

    // macOS application bundles.
    if cfg!(target_os = "macos") {
        let candidates = vec![
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
            "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
        ];

        // Also check ~/Applications
        let mut all = candidates.clone();
        if let Some(home) = dirs::home_dir() {
            for c in &candidates {
                let name = c.strip_prefix("/Applications/").unwrap_or(c);
                let p = home.join("Applications").join(name);
                all.push(Box::leak(p.to_string_lossy().into_owned().into_boxed_str()));
            }
        }

        for p in all {
            if Path::new(p).exists() {
                return Some(p.to_string());
            }
        }
    }

    // Linux: check common paths
    if cfg!(target_os = "linux") {
        let linux_paths = [
            "/usr/bin/google-chrome",
            "/usr/bin/google-chrome-stable",
            "/usr/bin/chromium",
            "/usr/bin/chromium-browser",
            "/snap/bin/chromium",
        ];
        for p in &linux_paths {
            if Path::new(p).exists() {
                return Some(p.to_string());
            }
        }
    }

    // Windows: check common install paths.
    if cfg!(target_os = "windows") {
        let program_files = std::env::var("ProgramFiles").unwrap_or_default();
        let program_files_x86 = std::env::var("ProgramFiles(x86)").unwrap_or_default();
        let local_app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();

        let windows_paths = vec![
            format!("{}\\Google\\Chrome\\Application\\chrome.exe", program_files),
            format!(
                "{}\\Google\\Chrome\\Application\\chrome.exe",
                program_files_x86
            ),
            format!(
                "{}\\Google\\Chrome\\Application\\chrome.exe",
                local_app_data
            ),
            format!("{}\\Chromium\\Application\\chrome.exe", program_files),
            format!(
                "{}\\BraveSoftware\\Brave-Browser\\Application\\brave.exe",
                program_files
            ),
            format!(
                "{}\\BraveSoftware\\Brave-Browser\\Application\\brave.exe",
                program_files_x86
            ),
            format!(
                "{}\\BraveSoftware\\Brave-Browser\\Application\\brave.exe",
                local_app_data
            ),
            format!(
                "{}\\Microsoft\\Edge\\Application\\msedge.exe",
                program_files_x86
            ),
            format!(
                "{}\\Microsoft\\Edge\\Application\\msedge.exe",
                program_files
            ),
        ];
        for p in &windows_paths {
            if Path::new(p).exists() {
                return Some(p.clone());
            }
        }
    }

    None
}
