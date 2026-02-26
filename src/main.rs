mod browser;
mod cli;
mod daemon;
mod plugins;
mod rpc;
mod watch;
mod web;

use anyhow::{Context, Result};
use clap::Parser;
use std::path::{Path, PathBuf};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("browsercli=info".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = cli::Cli::parse();
    let json_mode = cli.json;

    let result = run(cli);

    if let Err(e) = result {
        if json_mode {
            let err_json = serde_json::json!({
                "schema_version": rpc::types::SCHEMA_VERSION,
                "error": format!("{:#}", e),
            });
            println!("{}", serde_json::to_string_pretty(&err_json).unwrap());
        } else {
            eprintln!("Error: {:#}", e);
        }
        std::process::exit(1);
    }
}

fn run(cli: cli::Cli) -> Result<()> {
    match cli.command {
        cli::Commands::Start {
            dir,
            port,
            devtools_port,
            headless,
            no_app,
            no_stealth,
            window_size,
            browser_bin,
            restart,
            template,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_start(StartConfig {
                dir,
                port,
                devtools_port,
                headless,
                app: !no_app,
                stealth: !no_stealth,
                window_size,
                browser_bin,
                restart,
                json: cli.json,
                template,
            }))
        }
        cli::Commands::Serve {
            dir,
            port,
            devtools_port,
            headless,
            no_app,
            no_stealth,
            window_size,
            browser_bin,
            template,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_serve(ServeConfig {
                dir,
                port,
                devtools_port,
                headless,
                app: !no_app,
                stealth: !no_stealth,
                window_size,
                browser_bin,
                template,
            }))
        }
        cli::Commands::Daemon {
            state_dir,
            dir,
            port,
            devtools_port,
            headless,
            app,
            stealth,
            window_size,
            browser_bin,
            temp_dir,
            template,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(daemon::run_daemon(daemon::DaemonConfig {
                state_dir,
                serve_dir: dir,
                http_port: port,
                devtools_port,
                headless,
                app,
                window_size,
                browser_bin: browser_bin.unwrap_or_default(),
                stealth,
                temp_dir,
                watch: true,
                template,
            }))
        }
        cli::Commands::Status => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_status(cli.json))
        }
        cli::Commands::Stop => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_stop(cli.json))
        }
        cli::Commands::Focus => cmd_focus(),
        cli::Commands::Devtools => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_devtools(cli.json))
        }
        cli::Commands::Goto { path } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_goto(&path, cli.json))
        }
        cli::Commands::Eval { expression } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_eval(&expression, cli.json))
        }
        cli::Commands::Reload => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_reload(cli.json))
        }
        cli::Commands::Dom {
            action,
            selector,
            mode,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_dom(action, selector, mode, cli.json))
        }
        cli::Commands::Screenshot { selector, out } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_screenshot(selector, out, cli.json))
        }
        cli::Commands::Console {
            level,
            limit,
            clear,
        } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_console(level, limit, clear, cli.json))
        }
        cli::Commands::Network { limit, clear } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_network(limit, clear, cli.json))
        }
        cli::Commands::Perf => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(cmd_perf(cli.json))
        }
        cli::Commands::Plugin { action } => match action {
            cli::PluginAction::List => cmd_plugin_list(cli.json),
            cli::PluginAction::Init { name } => cmd_plugin_init(&name),
        },
    }
}

/// Returns the state directory.
/// On Unix: ~/.browsercli
/// On Windows: %LOCALAPPDATA%\browsercli
pub fn state_dir() -> Result<String> {
    #[cfg(windows)]
    let dir = dirs::data_local_dir()
        .context("could not determine local app data directory")?
        .join("browsercli");
    #[cfg(not(windows))]
    let dir = dirs::home_dir()
        .context("could not determine home directory")?
        .join(".browsercli");
    Ok(dir.to_string_lossy().to_string())
}

// --- Start command: spawn daemon in background ---

struct StartConfig {
    dir: Option<String>,
    port: u16,
    devtools_port: u16,
    headless: bool,
    app: bool,
    stealth: bool,
    window_size: String,
    browser_bin: Option<String>,
    restart: bool,
    json: bool,
    template: Option<String>,
}

async fn cmd_start(cfg: StartConfig) -> Result<()> {
    let StartConfig {
        dir,
        port,
        devtools_port,
        headless,
        app,
        stealth,
        window_size,
        browser_bin,
        restart,
        json,
        template,
    } = cfg;
    let sd = state_dir()?;
    let session_path = Path::new(&sd).join("session.json");

    // If already running, handle restart vs error.
    if session_path.exists() {
        if let Ok(data) = std::fs::read_to_string(&session_path) {
            if let Ok(sess) = serde_json::from_str::<serde_json::Value>(&data) {
                let pid = sess["pid"].as_u64().unwrap_or(0) as u32;
                if pid > 0 && process_alive(pid) {
                    if restart {
                        let socket = sess["socket_path"].as_str().unwrap_or("");
                        let token = sess["token"].as_str().unwrap_or("");
                        if !socket.is_empty() {
                            let client = rpc::RpcClient::new(socket, token);
                            let _ = client.stop().await;
                            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        }
                    } else {
                        if json {
                            let out = serde_json::json!({
                                "running": true,
                                "pid": pid,
                                "message": "already running (use --restart to replace)"
                            });
                            println!("{}", serde_json::to_string_pretty(&out)?);
                        } else {
                            eprintln!(
                                "browsercli is already running (PID {}). Use --restart to replace.",
                                pid
                            );
                        }
                        return Ok(());
                    }
                }
            }
        }
        let _ = std::fs::remove_file(&session_path);
    }

    // Resolve serve dir.
    let (serve_dir, is_temp) = match dir {
        Some(d) => {
            let p = PathBuf::from(&d);
            std::fs::create_dir_all(&p)?;
            (p.canonicalize()?.to_string_lossy().to_string(), false)
        }
        None => {
            let tmp = std::env::temp_dir().join(format!("browsercli-{}", std::process::id()));
            std::fs::create_dir_all(&tmp)?;
            (tmp.to_string_lossy().to_string(), true)
        }
    };

    std::fs::create_dir_all(&sd)?;

    // Spawn daemon process.
    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("daemon")
        .arg("--state-dir")
        .arg(&sd)
        .arg("--dir")
        .arg(&serve_dir)
        .arg("--port")
        .arg(port.to_string())
        .arg("--devtools-port")
        .arg(devtools_port.to_string())
        .arg("--window-size")
        .arg(&window_size);

    if headless {
        cmd.arg("--headless");
    }
    if app {
        cmd.arg("--app");
    }
    if stealth {
        cmd.arg("--stealth");
    }
    if let Some(ref bin) = browser_bin {
        cmd.arg("--browser-bin").arg(bin);
    }
    if is_temp {
        cmd.arg("--temp-dir");
    }
    if let Some(ref tpl) = template {
        cmd.arg("--template").arg(tpl);
    }

    // Redirect daemon stderr to a log file for diagnostics on startup failure.
    let log_path = Path::new(&sd).join("daemon.log");
    let log_file = std::fs::File::create(&log_path).context("failed to create daemon log file")?;

    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(log_file));

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }

    let child = cmd.spawn().context("failed to spawn daemon")?;
    let daemon_pid = child.id();

    // Wait for session.json to appear.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let sess: serde_json::Value = loop {
        if std::time::Instant::now() >= deadline {
            let log_content = std::fs::read_to_string(&log_path).unwrap_or_default();
            let detail = if log_content.trim().is_empty() {
                "no daemon output captured".to_string()
            } else {
                format!("daemon log:\n{}", log_content.trim())
            };
            anyhow::bail!("timed out waiting for daemon to start ({})", detail);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
        if let Ok(data) = std::fs::read_to_string(&session_path) {
            if let Ok(s) = serde_json::from_str::<serde_json::Value>(&data) {
                if s.get("http_port").is_some() {
                    break s;
                }
            }
        }
    };
    let http_port = sess["http_port"].as_u64().unwrap_or(0);

    if json {
        println!("{}", serde_json::to_string_pretty(&sess)?);
    } else {
        eprintln!("browsercli started (PID {})", daemon_pid);
        eprintln!("  URL:     http://127.0.0.1:{}/", http_port);
        eprintln!("  Serving: {}", serve_dir);
        if let Some(ws) = sess["devtools_ws_url"].as_str() {
            if !ws.is_empty() {
                eprintln!("  DevTools: {}", ws);
            }
        }
    }

    Ok(())
}

// --- Serve command: run in foreground ---

struct ServeConfig {
    dir: Option<String>,
    port: u16,
    devtools_port: u16,
    headless: bool,
    app: bool,
    stealth: bool,
    window_size: String,
    browser_bin: Option<String>,
    template: Option<String>,
}

async fn cmd_serve(cfg: ServeConfig) -> Result<()> {
    let ServeConfig {
        dir,
        port,
        devtools_port,
        headless,
        app,
        stealth,
        window_size,
        browser_bin,
        template,
    } = cfg;
    let sd = state_dir()?;

    let (serve_dir, is_temp) = match dir {
        Some(d) => {
            let p = PathBuf::from(&d);
            std::fs::create_dir_all(&p)?;
            (p.canonicalize()?.to_string_lossy().to_string(), false)
        }
        None => {
            let tmp = std::env::temp_dir().join(format!("browsercli-{}", std::process::id()));
            std::fs::create_dir_all(&tmp)?;
            (tmp.to_string_lossy().to_string(), true)
        }
    };

    daemon::run_daemon(daemon::DaemonConfig {
        state_dir: sd,
        serve_dir,
        http_port: port,
        devtools_port,
        headless,
        app,
        window_size,
        browser_bin: browser_bin.unwrap_or_default(),
        stealth,
        temp_dir: is_temp,
        watch: true,
        template,
    })
    .await
}

// --- Status ---

async fn cmd_status(json: bool) -> Result<()> {
    let (client, _sess, _sd) = rpc::client::must_client()?;
    let status = client.status().await?;

    if json {
        let mut val = serde_json::to_value(&status)?;
        val["schema_version"] = serde_json::json!(rpc::types::SCHEMA_VERSION);
        println!("{}", serde_json::to_string_pretty(&val)?);
    } else {
        println!("running:      {}", status.running);
        println!(
            "browser:      {}",
            if status.browser_alive {
                "alive"
            } else {
                "dead"
            }
        );
        println!("pid:          {}", status.pid);
        println!("dir:          {}", status.dir);
        println!(
            "http:         http://{}:{}/",
            status.http_addr, status.http_port
        );
        if !status.current_url.is_empty() {
            println!("current url:  {}", status.current_url);
        }
        if !status.title.is_empty() {
            println!("title:        {}", status.title);
        }
        if status.devtools_port > 0 {
            println!("devtools:     ws://127.0.0.1:{}/", status.devtools_port);
        }
        if !status.browser_bin.is_empty() {
            println!("browser bin:  {}", status.browser_bin);
        }
    }

    Ok(())
}

// --- Stop ---

async fn cmd_stop(json: bool) -> Result<()> {
    let (client, _sess, sd) = rpc::client::must_client()?;
    let resp = client.stop().await?;

    let _ = std::fs::remove_file(Path::new(&sd).join("session.json"));

    if json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        eprintln!("browsercli stopped");
    }

    Ok(())
}

// --- Focus ---

fn cmd_focus() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        // Read session.json to determine the actual browser being used.
        let app_name = detect_macos_app_name().unwrap_or_else(|| "Google Chrome".to_string());
        let script = format!(r#"tell application "{}" to activate"#, app_name);
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output();
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        eprintln!("focus is only supported on macOS");
    }
    #[cfg(target_os = "windows")]
    {
        eprintln!("focus is not yet supported on Windows");
    }
    Ok(())
}

/// Derive macOS application name from the browser binary path in session.json.
#[cfg(target_os = "macos")]
fn detect_macos_app_name() -> Option<String> {
    let sd = state_dir().ok()?;
    let session_path = Path::new(&sd).join("session.json");
    let data = std::fs::read_to_string(&session_path).ok()?;
    let sess: serde_json::Value = serde_json::from_str(&data).ok()?;
    let bin = sess["browser_bin"].as_str().unwrap_or("");

    // Extract app name from macOS .app bundle path.
    // e.g. "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser" -> "Brave Browser"
    if let Some(idx) = bin.find(".app/") {
        let prefix = &bin[..idx];
        let app_name = prefix.rsplit('/').next().unwrap_or("Google Chrome");
        return Some(app_name.to_string());
    }

    // Fallback: map common binary names to macOS app names.
    let name = Path::new(bin).file_name()?.to_str()?;
    match name {
        "google-chrome" | "google-chrome-stable" | "Google Chrome" => {
            Some("Google Chrome".to_string())
        }
        "chromium" | "chromium-browser" | "Chromium" => Some("Chromium".to_string()),
        "brave-browser" | "Brave Browser" => Some("Brave Browser".to_string()),
        "Microsoft Edge" | "microsoft-edge" => Some("Microsoft Edge".to_string()),
        _ => None,
    }
}

// --- Devtools ---

async fn cmd_devtools(json: bool) -> Result<()> {
    let (_client, sess, _sd) = rpc::client::must_client()?;
    let devtools_port = sess["devtools_port"].as_u64().unwrap_or(0);
    let devtools_ws = sess["devtools_ws_url"].as_str().unwrap_or("");

    if json {
        let out = serde_json::json!({
            "devtools_port": devtools_port,
            "devtools_ws_url": devtools_ws,
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else if !devtools_ws.is_empty() {
        println!("{}", devtools_ws);
    } else if devtools_port > 0 {
        println!("ws://127.0.0.1:{}/", devtools_port);
    } else {
        eprintln!("no DevTools info available");
    }

    Ok(())
}

// --- Goto ---

async fn cmd_goto(path: &str, json: bool) -> Result<()> {
    let (client, _sess, _sd) = rpc::client::must_client()?;
    let resp = client.goto(path).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        println!("{}", resp.url);
    }

    Ok(())
}

// --- Eval ---

async fn cmd_eval(expression: &str, json: bool) -> Result<()> {
    let (client, _sess, _sd) = rpc::client::must_client()?;
    let resp = client.eval(expression).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        let s = match &resp.value {
            serde_json::Value::String(s) => s.clone(),
            other => serde_json::to_string_pretty(other)?,
        };
        println!("{}", s);
    }

    Ok(())
}

// --- Reload ---

async fn cmd_reload(json: bool) -> Result<()> {
    let (client, _sess, _sd) = rpc::client::must_client()?;
    let resp = client.reload().await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        eprintln!("reloaded");
    }

    Ok(())
}

// --- Dom ---

async fn cmd_dom(
    action: Option<cli::DomAction>,
    selector: Vec<String>,
    mode: String,
    json: bool,
) -> Result<()> {
    let (client, _sess, _sd) = rpc::client::must_client()?;

    match action {
        Some(cli::DomAction::Query { selector, mode }) => {
            let resp = client.dom(&selector, &mode).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&resp)?);
            } else {
                println!("{}", resp.value);
            }
        }
        Some(cli::DomAction::All {
            selector,
            mode,
            limit,
        }) => {
            let resp = client.dom_all(&selector, &mode).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&resp)?);
            } else {
                let vals = if limit > 0 && resp.values.len() > limit {
                    &resp.values[..limit]
                } else {
                    &resp.values
                };
                for v in vals {
                    println!("{}", v);
                }
            }
        }
        Some(cli::DomAction::Attr { selector, name }) => {
            let resp = client.dom_attr(&selector, &name).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&resp)?);
            } else {
                println!("{}", resp.value.as_deref().unwrap_or("(null)"));
            }
        }
        Some(cli::DomAction::Click { selector }) => {
            let resp = client.dom_click(&selector).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&resp)?);
            } else {
                eprintln!("clicked");
            }
        }
        Some(cli::DomAction::Type {
            selector,
            text,
            clear,
        }) => {
            let resp = client.dom_type(&selector, &text, clear).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&resp)?);
            } else {
                eprintln!("typed");
            }
        }
        Some(cli::DomAction::Wait {
            selector,
            state,
            timeout,
        }) => {
            let timeout_ms = parse_duration_ms(&timeout);
            let resp = client.dom_wait(&selector, &state, timeout_ms).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&resp)?);
            } else {
                eprintln!("ok");
            }
        }
        None => {
            let sel = selector.join(" ");
            if sel.is_empty() {
                anyhow::bail!("usage: browsercli dom <selector> or browsercli dom <subcommand>");
            }
            let resp = client.dom(&sel, &mode).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&resp)?);
            } else {
                println!("{}", resp.value);
            }
        }
    }

    Ok(())
}

// --- Screenshot ---

async fn cmd_screenshot(selector: Option<String>, out: Option<String>, json: bool) -> Result<()> {
    let (client, _sess, _sd) = rpc::client::must_client()?;
    let resp = client.screenshot(selector.as_deref().unwrap_or("")).await?;

    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(&resp.base64)?;

    let output_path = out.unwrap_or_else(|| "screenshot.png".to_string());
    std::fs::write(&output_path, &bytes)?;

    if json {
        let out = serde_json::json!({
            "format": resp.format,
            "path": output_path,
            "size": bytes.len(),
        });
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        eprintln!("saved {}", output_path);
    }

    Ok(())
}

// --- Console ---

async fn cmd_console(level: Option<String>, limit: usize, clear: bool, json: bool) -> Result<()> {
    let (client, _sess, _sd) = rpc::client::must_client()?;
    let resp = client
        .console(level.as_deref().unwrap_or(""), limit, clear)
        .await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        for e in &resp.entries {
            println!("[{}] {}", e.level, e.text);
        }
        if resp.entries.is_empty() {
            eprintln!("(no console entries)");
        }
    }

    Ok(())
}

// --- Network ---

async fn cmd_network(limit: usize, clear: bool, json: bool) -> Result<()> {
    let (client, _sess, _sd) = rpc::client::must_client()?;
    let resp = client.network(limit, clear).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        for e in &resp.entries {
            println!(
                "{} {} {} [{}] ({}) {}ms",
                e.status, e.method, e.url, e.resource_type, e.mime_type, e.duration_ms
            );
        }
        if resp.entries.is_empty() {
            eprintln!("(no network entries)");
        }
    }

    Ok(())
}

// --- Perf ---

async fn cmd_perf(json: bool) -> Result<()> {
    let (client, _sess, _sd) = rpc::client::must_client()?;
    let resp = client.perf().await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        println!("DOMContentLoaded: {:.0}ms", resp.dom_content_loaded_ms);
        println!("Load:             {:.0}ms", resp.load_event_ms);
    }

    Ok(())
}

// --- Plugin commands ---

fn cmd_plugin_list(json: bool) -> Result<()> {
    let sd = state_dir()?;
    let plugins_root = PathBuf::from(&sd).join("plugins");
    let registry = plugins::PluginRegistry::new(&plugins_root)
        .unwrap_or_else(|_| plugins::PluginRegistry::empty());

    let summary = registry.summary();

    if json {
        let plugins_info: Vec<rpc::PluginInfo> = summary
            .iter()
            .map(|s| rpc::PluginInfo {
                name: s.name.clone(),
                version: s.version.clone(),
                description: s.description.clone(),
                templates: s.templates.clone(),
                hooks: s.hooks.clone(),
                rpc_endpoints: s.rpc_endpoints.clone(),
            })
            .collect();
        let resp = rpc::PluginListResponse {
            plugins: plugins_info,
        };
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else if summary.is_empty() {
        eprintln!("no plugins installed");
        eprintln!("  plugins directory: {}", plugins_root.display());
        eprintln!("  create one with:   browsercli plugin init <name>");
    } else {
        for p in &summary {
            println!("{} v{}", p.name, p.version);
            if !p.description.is_empty() {
                println!("  {}", p.description);
            }
            if !p.templates.is_empty() {
                println!("  templates: {}", p.templates.join(", "));
            }
            if !p.hooks.is_empty() {
                println!("  hooks:     {}", p.hooks.join(", "));
            }
            if !p.rpc_endpoints.is_empty() {
                println!("  endpoints: {}", p.rpc_endpoints.join(", "));
            }
        }
    }

    Ok(())
}

fn cmd_plugin_init(name: &str) -> Result<()> {
    // Validate name.
    if name.is_empty()
        || name.len() > 64
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        anyhow::bail!(
            "invalid plugin name '{}': must be 1-64 alphanumeric/hyphen/underscore characters",
            name
        );
    }

    let sd = state_dir()?;
    let plugin_dir = PathBuf::from(&sd).join("plugins").join(name);

    if plugin_dir.exists() {
        anyhow::bail!("plugin directory already exists: {}", plugin_dir.display());
    }

    // Create scaffold.
    std::fs::create_dir_all(plugin_dir.join("templates").join(name))?;
    std::fs::create_dir_all(plugin_dir.join("handlers"))?;
    std::fs::create_dir_all(plugin_dir.join("hooks"))?;

    // plugin.json
    let manifest = serde_json::json!({
        "name": name,
        "version": "0.1.0",
        "description": format!("{} plugin for browsercli", name),
        "templates": {
            name: {
                "source": format!("templates/{}/", name),
                "entrypoint": "index.html",
                "description": format!("{} template", name)
            }
        },
        "hooks": {
            "on_daemon_start": "hooks/on_start.sh"
        },
        "rpc": {
            "endpoints": [
                {
                    "path": format!("/x/{}/hello", name),
                    "handler": "handlers/hello.sh",
                    "method": "POST",
                    "description": "Example endpoint"
                }
            ]
        }
    });
    std::fs::write(
        plugin_dir.join("plugin.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    // Example template
    let index_html = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{name}</title>
  <style>
    body {{ font-family: system-ui, sans-serif; max-width: 800px; margin: 2rem auto; padding: 0 1rem; }}
    h1 {{ color: #333; }}
  </style>
</head>
<body>
  <h1>{name}</h1>
  <p>This is a template from the <strong>{name}</strong> plugin.</p>
</body>
</html>"#,
        name = name
    );
    std::fs::write(
        plugin_dir.join("templates").join(name).join("index.html"),
        index_html,
    )?;

    // Example hook
    let hook = format!(
        "#!/bin/sh\necho \"[{name}] daemon started — serve dir: $BROWSERCLI_DIR\"\n",
        name = name
    );
    std::fs::write(plugin_dir.join("hooks").join("on_start.sh"), &hook)?;

    // Example handler
    let handler = format!(
        r#"#!/bin/sh
echo '{{"plugin":"{name}","message":"hello from {name}!"}}'
"#,
        name = name
    );
    std::fs::write(plugin_dir.join("handlers").join("hello.sh"), &handler)?;

    // Make scripts executable on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            plugin_dir.join("hooks").join("on_start.sh"),
            std::fs::Permissions::from_mode(0o755),
        )?;
        std::fs::set_permissions(
            plugin_dir.join("handlers").join("hello.sh"),
            std::fs::Permissions::from_mode(0o755),
        )?;
    }

    eprintln!("created plugin scaffold: {}", plugin_dir.display());
    eprintln!("  manifest:  {}", plugin_dir.join("plugin.json").display());
    eprintln!(
        "  template:  {}",
        plugin_dir.join("templates").join(name).display()
    );
    eprintln!(
        "  hook:      {}",
        plugin_dir.join("hooks").join("on_start.sh").display()
    );
    eprintln!(
        "  handler:   {}",
        plugin_dir.join("handlers").join("hello.sh").display()
    );

    Ok(())
}

// --- Helpers ---

fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return false;
        }
        unsafe { CloseHandle(handle) };
        true
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        false
    }
}

fn parse_duration_ms(s: &str) -> u64 {
    let s = s.trim();
    if let Some(secs) = s.strip_suffix('s') {
        let secs = secs.trim();
        if !secs.ends_with('m') {
            if let Ok(n) = secs.parse::<f64>() {
                return (n * 1000.0) as u64;
            }
        }
    }
    if let Some(ms) = s.strip_suffix("ms") {
        if let Ok(n) = ms.trim().parse::<u64>() {
            return n;
        }
    }
    s.parse::<u64>().unwrap_or(10000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_seconds() {
        assert_eq!(parse_duration_ms("10s"), 10000);
        assert_eq!(parse_duration_ms("1s"), 1000);
        assert_eq!(parse_duration_ms("0.5s"), 500);
        assert_eq!(parse_duration_ms("2.5s"), 2500);
    }

    #[test]
    fn parse_duration_milliseconds() {
        assert_eq!(parse_duration_ms("500ms"), 500);
        assert_eq!(parse_duration_ms("1000ms"), 1000);
        assert_eq!(parse_duration_ms("100ms"), 100);
    }

    #[test]
    fn parse_duration_raw_number() {
        assert_eq!(parse_duration_ms("5000"), 5000);
    }

    #[test]
    fn parse_duration_default() {
        assert_eq!(parse_duration_ms("invalid"), 10000);
        assert_eq!(parse_duration_ms(""), 10000);
    }

    #[test]
    fn parse_duration_whitespace() {
        assert_eq!(parse_duration_ms("  10s  "), 10000);
        assert_eq!(parse_duration_ms("  500ms  "), 500);
    }

    #[test]
    fn state_dir_returns_path() {
        let sd = state_dir().unwrap();
        // On Unix the state dir ends with ".browsercli";
        // on Windows it ends with "browsercli" (inside %LOCALAPPDATA%).
        assert!(
            sd.ends_with(".browsercli") || sd.ends_with("browsercli"),
            "unexpected state_dir: {sd}"
        );
    }
}
