use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::TcpListener;
#[cfg(unix)]
use tokio::net::UnixListener;

use crate::browser::controller::{BrowserController, LaunchOptions};
use crate::plugins;
use crate::rpc;
use crate::web;

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub state_dir: String,
    pub serve_dir: String,
    pub http_port: u16,
    pub devtools_port: u16,
    pub headless: bool,
    pub app: bool,
    pub window_size: String,
    pub browser_bin: String,
    pub stealth: bool,
    pub temp_dir: bool,
    pub watch: bool,
    pub template: Option<String>,
}

struct AppState {
    browser: Arc<BrowserController>,
    config: DaemonConfig,
    token: String,
    http_port: u16,
    base_url: String,
    plugin_registry: Arc<plugins::PluginRegistry>,
    plugin_executor: Arc<plugins::ScriptExecutor>,
}

pub async fn run_daemon(cfg: DaemonConfig) -> Result<()> {
    if cfg.state_dir.is_empty() {
        anyhow::bail!("missing state dir");
    }
    if cfg.serve_dir.is_empty() {
        anyhow::bail!("missing serve dir");
    }

    tokio::fs::create_dir_all(&cfg.state_dir).await?;

    // Load plugins.
    let plugins_root = PathBuf::from(&cfg.state_dir).join("plugins");
    let plugin_registry = Arc::new(plugins::PluginRegistry::new(&plugins_root).unwrap_or_else(
        |e| {
            tracing::warn!(error = %e, "failed to load plugins, continuing without plugins");
            plugins::PluginRegistry::empty()
        },
    ));
    let plugin_executor = Arc::new(plugins::ScriptExecutor::with_default_timeout());

    // Apply template if requested.
    // Check built-in templates first, then fall back to plugin registry.
    if let Some(ref tpl_name) = cfg.template {
        let dest = PathBuf::from(&cfg.serve_dir);
        let applied = crate::builtin_templates::apply_builtin_template(tpl_name, &dest).await?;
        if !applied {
            // Fall back to plugin-provided templates.
            let mut available: Vec<&str> = crate::builtin_templates::list_builtin_templates();
            available.extend(plugin_registry.list_templates());
            let tpl = plugin_registry.get_template(tpl_name).ok_or_else(|| {
                anyhow::anyhow!(
                    "template '{}' not found (available: {})",
                    tpl_name,
                    available.join(", ")
                )
            })?;
            plugins::apply_template(tpl, &dest).await?;
        }
    }

    #[cfg(unix)]
    let socket_path = PathBuf::from(&cfg.state_dir).join("browsercli.sock");
    #[cfg(unix)]
    let _ = tokio::fs::remove_file(&socket_path).await;

    // Generate random token.
    let token = generate_token();

    // HTTP static file server.
    let serve_dir = PathBuf::from(&cfg.serve_dir);
    let static_handler = Arc::new(web::StaticHandler::new(&serve_dir));

    let http_listener = TcpListener::bind(format!("127.0.0.1:{}", cfg.http_port)).await?;
    let actual_port = http_listener.local_addr()?.port();
    let base_url = format!("http://127.0.0.1:{}/", actual_port);

    // Set welcome page provider.
    let base_url_clone = base_url.clone();
    let serve_dir_str = cfg.serve_dir.clone();
    let watch_enabled = cfg.watch;
    let app_mode = cfg.app && !cfg.headless;
    static_handler.set_welcome(move || web::WelcomeData {
        serve_dir: serve_dir_str.clone(),
        http_url: base_url_clone.clone(),
        auto_reload: watch_enabled,
        app_mode,
        ..Default::default()
    });

    // Launch browser.
    let profile_dir = PathBuf::from(&cfg.state_dir).join("chrome-profile");
    let _ = tokio::fs::remove_dir_all(&profile_dir).await;
    tokio::fs::create_dir_all(&profile_dir).await?;

    let browser_bin = if cfg.browser_bin.is_empty() {
        crate::browser::find::find_chromium_binary()
            .context(browser_not_found_message())?
    } else {
        cfg.browser_bin.clone()
    };

    let devtools_port = if cfg.devtools_port == 0 {
        pick_free_port().await?
    } else {
        cfg.devtools_port
    };

    let window_size = if cfg.window_size.is_empty() {
        "1280,720".to_string()
    } else {
        cfg.window_size.clone()
    };

    let controller = BrowserController::launch(LaunchOptions {
        browser_bin: browser_bin.clone(),
        headless: cfg.headless,
        user_data_dir: profile_dir.to_string_lossy().to_string(),
        devtools_port,
        start_url: base_url.clone(),
        app_mode: cfg.app && !cfg.headless,
        window_size,
        stealth: cfg.stealth,
    })
    .await
    .context("launch browser")?;

    // Navigate to base URL.
    controller.navigate(&base_url).await.ok();

    let state = Arc::new(AppState {
        browser: controller.clone(),
        config: cfg.clone(),
        token: token.clone(),
        http_port: actual_port,
        base_url: base_url.clone(),
        plugin_registry: plugin_registry.clone(),
        plugin_executor: plugin_executor.clone(),
    });

    // Dispatch on_daemon_start hooks.
    {
        let base_env = build_plugin_env(
            &token,
            actual_port,
            &cfg.serve_dir,
            &base_url,
            &cfg.state_dir,
        );
        plugins::dispatch_hook(
            &plugin_registry,
            plugins::HookEvent::DaemonStart,
            &plugin_executor,
            &base_env,
        );
    }

    // Start RPC server.
    #[cfg(unix)]
    let rpc_listener = UnixListener::bind(&socket_path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(windows)]
    let rpc_listener = TcpListener::bind("127.0.0.1:0").await?;
    #[cfg(windows)]
    let rpc_port = rpc_listener.local_addr()?.port();

    // Save session state.
    let mut session = serde_json::json!({
        "pid": std::process::id(),
        "started_at": chrono_like_now(),
        "dir": cfg.serve_dir,
        "http_addr": "127.0.0.1",
        "http_port": actual_port,
        "token": token,
        "headless": cfg.headless,
        "browser_pid": controller.browser_pid(),
        "devtools_port": controller.devtools_port(),
        "devtools_ws_url": controller.devtools_ws_url(),
        "browser_bin": controller.browser_binary(),
    });
    #[cfg(unix)]
    {
        session["socket_path"] = serde_json::json!(socket_path.to_string_lossy().to_string());
    }
    #[cfg(windows)]
    {
        session["rpc_port"] = serde_json::json!(rpc_port);
    }
    save_session(&cfg.state_dir, &session).await?;

    // File watcher for auto-reload.
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    if cfg.watch {
        let browser_for_watch = controller.clone();
        let watch_dir = serve_dir.clone();
        let watch_rx = shutdown_rx.clone();
        tokio::spawn(async move {
            let _ = crate::watch::watch_recursive(&watch_dir, watch_rx, move || {
                let browser = browser_for_watch.clone();
                tokio::task::block_in_place(|| {
                    let rt = tokio::runtime::Handle::current();
                    rt.block_on(async {
                        let _ = browser.reload().await;
                    });
                });
            })
            .await;
        });
    }

    // Spawn HTTP file server.
    let static_handler_for_http = static_handler.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = http_listener.accept().await {
            let handler = static_handler_for_http.clone();
            let io = hyper_util::rt::TokioIo::new(stream);
            tokio::spawn(async move {
                let service = service_fn(move |req: Request<Incoming>| {
                    let h = handler.clone();
                    async move {
                        let path = req.uri().path().to_string();
                        let (status, content_type, body) = h.handle_request(&path).await;
                        let resp = Response::builder()
                            .status(status)
                            .header("content-type", content_type)
                            .body(Full::new(Bytes::from(body)))
                            .unwrap();
                        Ok::<_, hyper::Error>(resp)
                    }
                });
                let _ = http1::Builder::new().serve_connection(io, service).await;
            });
        }
    });

    // Spawn RPC server.
    let state_for_rpc = state.clone();
    let (stop_tx, mut stop_rx) = tokio::sync::mpsc::channel::<()>(1);
    tokio::spawn(async move {
        loop {
            let (stream, _) = match rpc_listener.accept().await {
                Ok(conn) => conn,
                Err(_) => break,
            };
            let st = state_for_rpc.clone();
            let stop = stop_tx.clone();
            let io = hyper_util::rt::TokioIo::new(stream);
            tokio::spawn(async move {
                let service = service_fn(move |req: Request<Incoming>| {
                    let st = st.clone();
                    let stop = stop.clone();
                    async move { handle_rpc(req, st, stop).await }
                });
                let _ = http1::Builder::new().serve_connection(io, service).await;
            });
        }
    });

    // Wait for shutdown signals.
    #[cfg(unix)]
    {
        use tokio::signal::unix::SignalKind;
        let mut sigint = tokio::signal::unix::signal(SignalKind::interrupt())?;
        let mut sigterm = tokio::signal::unix::signal(SignalKind::terminate())?;
        tokio::select! {
            _ = stop_rx.recv() => {}
            _ = sigint.recv() => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(windows)]
    {
        tokio::select! {
            _ = stop_rx.recv() => {}
            _ = tokio::signal::ctrl_c() => {}
        }
    }

    // Dispatch on_daemon_stop hooks (with timeout).
    {
        let base_env = build_plugin_env(
            &token,
            actual_port,
            &cfg.serve_dir,
            &base_url,
            &cfg.state_dir,
        );
        plugins::dispatch_hook(
            &plugin_registry,
            plugins::HookEvent::DaemonStop,
            &plugin_executor,
            &base_env,
        );
        // Give hooks a moment to fire.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    // Shutdown.
    let _ = shutdown_tx.send(true);
    let _ = controller.close().await;
    #[cfg(unix)]
    let _ = tokio::fs::remove_file(&socket_path).await;
    let _ = remove_session(&cfg.state_dir).await;

    if cfg.temp_dir {
        let _ = tokio::fs::remove_dir_all(&cfg.serve_dir).await;
    }

    Ok(())
}

async fn handle_rpc(
    req: Request<Incoming>,
    state: Arc<AppState>,
    stop_tx: tokio::sync::mpsc::Sender<()>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    // Auth check.
    if !state.token.is_empty() {
        let auth = req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if auth != format!("Bearer {}", state.token) {
            return Ok(Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .body(Full::new(Bytes::from("unauthorized")))
                .unwrap());
        }
    }

    let path = req.uri().path().to_string();
    let body_bytes = http_body_util::BodyExt::collect(req.into_body())
        .await
        .map(|c| c.to_bytes())
        .unwrap_or_default();

    let (status, json) = match path.as_str() {
        "/version" => (
            200,
            serde_json::json!({
                "rpc_version": rpc::types::RPC_VERSION,
                "schema_version": rpc::types::SCHEMA_VERSION,
            }),
        ),
        "/status" => {
            let loc = state.browser.location().await.unwrap_or_default();
            let title = state.browser.title().await.unwrap_or_default();
            let resp = rpc::StatusResponse {
                running: true,
                browser_alive: state.browser.alive().await,
                pid: std::process::id(),
                dir: state.config.serve_dir.clone(),
                http_addr: "127.0.0.1".to_string(),
                http_port: state.http_port,
                current_url: loc,
                title,
                headless: state.config.headless,
                browser_pid: state.browser.browser_pid(),
                devtools_port: state.browser.devtools_port(),
                devtools_ws_url: state.browser.devtools_ws_url().to_string(),
                browser_bin: state.browser.browser_binary().to_string(),
                ..Default::default()
            };
            (200, serde_json::to_value(resp).unwrap())
        }
        "/goto" => match serde_json::from_slice::<rpc::GotoRequest>(&body_bytes) {
            Ok(req) => {
                let u = normalize_url(&state.base_url, &req.url);
                match state.browser.navigate(&u).await {
                    Ok((loc, title)) => (
                        200,
                        serde_json::to_value(rpc::GotoResponse { url: loc, title }).unwrap(),
                    ),
                    Err(e) => (500, serde_json::json!({"error": e.to_string()})),
                }
            }
            Err(e) => (400, serde_json::json!({"error": e.to_string()})),
        },
        "/eval" => match serde_json::from_slice::<rpc::EvalRequest>(&body_bytes) {
            Ok(req) => match state.browser.eval(&req.expression).await {
                Ok(val) => (
                    200,
                    serde_json::to_value(rpc::EvalResponse { value: val }).unwrap(),
                ),
                Err(e) => (500, serde_json::json!({"error": e.to_string()})),
            },
            Err(e) => (400, serde_json::json!({"error": e.to_string()})),
        },
        "/reload" => match state.browser.reload().await {
            Ok(()) => (
                200,
                serde_json::to_value(rpc::ReloadResponse { ok: true }).unwrap(),
            ),
            Err(e) => (500, serde_json::json!({"error": e.to_string()})),
        },
        "/dom" => match serde_json::from_slice::<rpc::DomRequest>(&body_bytes) {
            Ok(req) => {
                let mode = if req.mode.is_empty() {
                    "outer_html"
                } else {
                    &req.mode
                };
                let result = match mode {
                    "text" => state.browser.text(&req.selector).await,
                    _ => state.browser.outer_html(&req.selector).await,
                };
                match result {
                    Ok(val) => (
                        200,
                        serde_json::to_value(rpc::DomResponse {
                            selector: req.selector,
                            mode: mode.to_string(),
                            value: val,
                        })
                        .unwrap(),
                    ),
                    Err(e) => (500, serde_json::json!({"error": e.to_string()})),
                }
            }
            Err(e) => (400, serde_json::json!({"error": e.to_string()})),
        },
        "/dom/all" => match serde_json::from_slice::<rpc::DomAllRequest>(&body_bytes) {
            Ok(req) => {
                let mode = if req.mode.is_empty() {
                    "outer_html"
                } else {
                    &req.mode
                };
                match state.browser.query_all(&req.selector, mode).await {
                    Ok(vals) => (
                        200,
                        serde_json::to_value(rpc::DomAllResponse {
                            selector: req.selector,
                            mode: mode.to_string(),
                            values: vals,
                        })
                        .unwrap(),
                    ),
                    Err(e) => (500, serde_json::json!({"error": e.to_string()})),
                }
            }
            Err(e) => (400, serde_json::json!({"error": e.to_string()})),
        },
        "/dom/attr" => match serde_json::from_slice::<rpc::DomAttrRequest>(&body_bytes) {
            Ok(req) => match state.browser.attr(&req.selector, &req.name).await {
                Ok(val) => (
                    200,
                    serde_json::to_value(rpc::DomAttrResponse {
                        selector: req.selector,
                        name: req.name,
                        value: val,
                    })
                    .unwrap(),
                ),
                Err(e) => {
                    let status = if e.to_string().contains("element not found") {
                        404
                    } else {
                        500
                    };
                    (status, serde_json::json!({"error": e.to_string()}))
                }
            },
            Err(e) => (400, serde_json::json!({"error": e.to_string()})),
        },
        "/dom/click" => match serde_json::from_slice::<rpc::DomClickRequest>(&body_bytes) {
            Ok(req) => match state.browser.click(&req.selector).await {
                Ok(()) => (
                    200,
                    serde_json::to_value(rpc::DomClickResponse { ok: true }).unwrap(),
                ),
                Err(e) => (500, serde_json::json!({"error": e.to_string()})),
            },
            Err(e) => (400, serde_json::json!({"error": e.to_string()})),
        },
        "/dom/type" => match serde_json::from_slice::<rpc::DomTypeRequest>(&body_bytes) {
            Ok(req) => match state
                .browser
                .type_text(&req.selector, &req.text, req.clear)
                .await
            {
                Ok(()) => (
                    200,
                    serde_json::to_value(rpc::DomTypeResponse { ok: true }).unwrap(),
                ),
                Err(e) => (500, serde_json::json!({"error": e.to_string()})),
            },
            Err(e) => (400, serde_json::json!({"error": e.to_string()})),
        },
        "/dom/wait" => match serde_json::from_slice::<rpc::DomWaitRequest>(&body_bytes) {
            Ok(req) => {
                let state_str = if req.state.is_empty() {
                    "visible"
                } else {
                    &req.state
                };
                let timeout = if req.timeout_ms > 0 {
                    std::time::Duration::from_millis(req.timeout_ms)
                } else {
                    std::time::Duration::from_secs(10)
                };
                match state.browser.wait(&req.selector, state_str, timeout).await {
                    Ok(()) => (
                        200,
                        serde_json::to_value(rpc::DomWaitResponse {
                            ok: true,
                            state: state_str.to_string(),
                        })
                        .unwrap(),
                    ),
                    Err(e) => (500, serde_json::json!({"error": e.to_string()})),
                }
            }
            Err(e) => (400, serde_json::json!({"error": e.to_string()})),
        },
        "/screenshot" => match serde_json::from_slice::<rpc::ScreenshotRequest>(&body_bytes) {
            Ok(req) => match state.browser.screenshot(&req.selector).await {
                Ok(buf) => {
                    use base64::Engine;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
                    (
                        200,
                        serde_json::to_value(rpc::ScreenshotResponse {
                            format: "png".to_string(),
                            base64: b64,
                        })
                        .unwrap(),
                    )
                }
                Err(e) => (500, serde_json::json!({"error": e.to_string()})),
            },
            Err(e) => (400, serde_json::json!({"error": e.to_string()})),
        },
        // Beyond Go version: console capture
        "/console" => match serde_json::from_slice::<rpc::ConsoleRequest>(&body_bytes) {
            Ok(req) => {
                let entries = state
                    .browser
                    .get_console_entries(&req.level, req.limit, req.clear)
                    .await;
                (
                    200,
                    serde_json::to_value(rpc::ConsoleResponse { entries }).unwrap(),
                )
            }
            Err(e) => (400, serde_json::json!({"error": e.to_string()})),
        },
        // Beyond Go version: network log
        "/network" => match serde_json::from_slice::<rpc::NetworkRequest>(&body_bytes) {
            Ok(req) => {
                let entries = state
                    .browser
                    .get_network_entries(req.limit, req.clear)
                    .await;
                (
                    200,
                    serde_json::to_value(rpc::NetworkResponse { entries }).unwrap(),
                )
            }
            Err(e) => (400, serde_json::json!({"error": e.to_string()})),
        },
        // Beyond Go version: performance metrics
        "/perf" => match state.browser.get_perf_metrics().await {
            Ok((dcl, load)) => (
                200,
                serde_json::to_value(rpc::PerfResponse {
                    dom_content_loaded_ms: dcl,
                    load_event_ms: load,
                })
                .unwrap(),
            ),
            Err(e) => (500, serde_json::json!({"error": e.to_string()})),
        },
        "/stop" => {
            let resp = serde_json::to_value(rpc::StopResponse { ok: true }).unwrap();
            let stop = stop_tx.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                let _ = stop.send(()).await;
            });
            (200, resp)
        }
        // Plugin system: list installed plugins
        "/plugins" => {
            let summary = state.plugin_registry.summary();
            let plugins: Vec<rpc::PluginInfo> = summary
                .into_iter()
                .map(|s| rpc::PluginInfo {
                    name: s.name,
                    version: s.version,
                    description: s.description,
                    templates: s.templates,
                    hooks: s.hooks,
                    rpc_endpoints: s.rpc_endpoints,
                })
                .collect();
            (
                200,
                serde_json::to_value(rpc::PluginListResponse { plugins }).unwrap(),
            )
        }
        // Plugin system: custom RPC endpoints
        p if p.starts_with("/x/") => {
            match state.plugin_registry.get_rpc_handler(p) {
                Some(handler) => {
                    let base_env = build_plugin_env(
                        &state.token,
                        state.http_port,
                        &state.config.serve_dir,
                        &state.base_url,
                        &state.config.state_dir,
                    );
                    let mut env = base_env;
                    env.insert(
                        "BROWSERCLI_PLUGIN_NAME".to_string(),
                        handler.plugin_name.clone(),
                    );

                    let ctx = plugins::ExecutionContext {
                        env_vars: env,
                        stdin: if body_bytes.is_empty() {
                            None
                        } else {
                            Some(body_bytes.to_vec())
                        },
                    };

                    match state
                        .plugin_executor
                        .execute(&handler.script_path, &ctx)
                        .await
                    {
                        Ok(result) => {
                            if result.timed_out {
                                (
                                    504,
                                    serde_json::json!({"error": "plugin handler timed out"}),
                                )
                            } else if result.exit_code != 0 {
                                let stderr = String::from_utf8_lossy(&result.stderr);
                                (
                                    500,
                                    serde_json::json!({
                                        "error": format!("plugin handler exited with code {}", result.exit_code),
                                        "stderr": stderr.trim()
                                    }),
                                )
                            } else {
                                // Try to parse stdout as JSON; fall back to wrapping in {"output": ...}.
                                match serde_json::from_slice::<serde_json::Value>(&result.stdout) {
                                    Ok(val) => (200, val),
                                    Err(_) => {
                                        let text = String::from_utf8_lossy(&result.stdout);
                                        (200, serde_json::json!({"output": text.trim()}))
                                    }
                                }
                            }
                        }
                        Err(e) => (500, serde_json::json!({"error": e.to_string()})),
                    }
                }
                None => (
                    404,
                    serde_json::json!({"error": format!("no plugin handler for {}", p)}),
                ),
            }
        }
        _ => (404, serde_json::json!({"error": "not found"})),
    };

    let body = serde_json::to_vec(&json).unwrap_or_default();
    Ok(Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(body)))
        .unwrap())
}

/// Build base environment variables for plugin script execution.
fn build_plugin_env(
    token: &str,
    http_port: u16,
    serve_dir: &str,
    base_url: &str,
    state_dir: &str,
) -> std::collections::HashMap<String, String> {
    let mut env = std::collections::HashMap::new();
    env.insert("BROWSERCLI_TOKEN".to_string(), token.to_string());
    env.insert("BROWSERCLI_HTTP_PORT".to_string(), http_port.to_string());
    env.insert("BROWSERCLI_DIR".to_string(), serve_dir.to_string());
    env.insert("BROWSERCLI_BASE_URL".to_string(), base_url.to_string());
    env.insert("BROWSERCLI_STATE_DIR".to_string(), state_dir.to_string());
    env
}

pub(crate) fn normalize_url(base_url: &str, input: &str) -> String {
    let s = input.trim();
    if s.is_empty() {
        return base_url.to_string();
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        return s.to_string();
    }
    let mut path = s.to_string();
    if !path.starts_with('/') {
        path = format!("/{}", path);
    }
    format!("{}{}", base_url.trim_end_matches('/'), path)
}

pub(crate) fn generate_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.gen();
    hex::encode(bytes)
}

async fn pick_free_port() -> Result<u16> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    Ok(listener.local_addr()?.port())
}

async fn save_session(state_dir: &str, session: &serde_json::Value) -> Result<()> {
    let dir = Path::new(state_dir);
    tokio::fs::create_dir_all(dir).await?;
    let path = dir.join("session.json");
    let tmp = dir.join("session.json.tmp");
    let data = serde_json::to_string_pretty(session)?;
    tokio::fs::write(&tmp, &data).await?;
    // Restrict permissions before renaming into place (contains auth token).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = tokio::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600)).await;
    }
    tokio::fs::rename(&tmp, &path).await?;
    Ok(())
}

async fn remove_session(state_dir: &str) -> Result<()> {
    let path = Path::new(state_dir).join("session.json");
    let _ = tokio::fs::remove_file(&path).await;
    Ok(())
}

/// Build a detailed error message when no Chromium browser is found.
fn browser_not_found_message() -> String {
    let mut msg = String::from("could not find a Chromium-based browser\n\n");
    msg.push_str("browsercli requires Chrome, Chromium, Brave, or Edge.\n\n");

    if cfg!(target_os = "macos") {
        msg.push_str("Install options (macOS):\n");
        msg.push_str("  brew install --cask google-chrome\n");
        msg.push_str("  brew install --cask chromium\n");
        msg.push_str("  brew install --cask brave-browser\n");
    } else if cfg!(target_os = "linux") {
        msg.push_str("Install options (Linux):\n");
        msg.push_str("  sudo apt install chromium-browser        (Ubuntu/Debian)\n");
        msg.push_str("  sudo dnf install chromium                (Fedora)\n");
        msg.push_str("  sudo pacman -S chromium                  (Arch)\n");
        msg.push_str("  snap install chromium                    (Snap)\n");
    } else if cfg!(target_os = "windows") {
        msg.push_str("Install options (Windows):\n");
        msg.push_str("  winget install Google.Chrome\n");
        msg.push_str("  choco install googlechrome\n");
    }

    msg.push_str("\nOr specify a custom binary path:\n");
    msg.push_str("  browsercli start --browser-bin /path/to/chrome\n\n");
    msg.push_str("You can also set the CHROME_BIN environment variable.\n");
    msg.push_str("See: https://github.com/justinhuangcode/browsercli#troubleshooting");
    msg
}

fn chrono_like_now() -> String {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s", now.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_url_empty_returns_base() {
        assert_eq!(
            normalize_url("http://127.0.0.1:8080/", ""),
            "http://127.0.0.1:8080/"
        );
    }

    #[test]
    fn normalize_url_full_url_passthrough() {
        assert_eq!(
            normalize_url("http://127.0.0.1:8080/", "https://example.com/page"),
            "https://example.com/page"
        );
        assert_eq!(
            normalize_url("http://127.0.0.1:8080/", "http://other.host/"),
            "http://other.host/"
        );
    }

    #[test]
    fn normalize_url_absolute_path() {
        assert_eq!(
            normalize_url("http://127.0.0.1:8080/", "/about"),
            "http://127.0.0.1:8080/about"
        );
        assert_eq!(
            normalize_url("http://127.0.0.1:8080/", "/foo/bar"),
            "http://127.0.0.1:8080/foo/bar"
        );
    }

    #[test]
    fn normalize_url_relative_path() {
        assert_eq!(
            normalize_url("http://127.0.0.1:8080/", "page.html"),
            "http://127.0.0.1:8080/page.html"
        );
    }

    #[test]
    fn normalize_url_trims_whitespace() {
        assert_eq!(
            normalize_url("http://127.0.0.1:8080/", "  /about  "),
            "http://127.0.0.1:8080/about"
        );
    }

    #[test]
    fn normalize_url_trailing_slash_handling() {
        assert_eq!(
            normalize_url("http://127.0.0.1:8080", "/about"),
            "http://127.0.0.1:8080/about"
        );
        assert_eq!(
            normalize_url("http://127.0.0.1:8080/", "/about"),
            "http://127.0.0.1:8080/about"
        );
    }

    #[test]
    fn generate_token_format() {
        let token = generate_token();
        assert_eq!(token.len(), 32); // 16 bytes = 32 hex chars
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_token_uniqueness() {
        let t1 = generate_token();
        let t2 = generate_token();
        assert_ne!(t1, t2);
    }

    #[tokio::test]
    async fn pick_free_port_returns_valid_port() {
        let port = pick_free_port().await.unwrap();
        assert!(port > 0);
    }

    #[tokio::test]
    async fn pick_free_port_unique() {
        let p1 = pick_free_port().await.unwrap();
        let p2 = pick_free_port().await.unwrap();
        // Ports should be different (extremely likely, not guaranteed).
        // Just check both are valid.
        assert!(p1 > 0);
        assert!(p2 > 0);
    }

    #[tokio::test]
    async fn save_and_remove_session() {
        let tmp = std::env::temp_dir().join(format!("browsercli-test-{}", std::process::id()));
        let _ = tokio::fs::create_dir_all(&tmp).await;

        let session = serde_json::json!({"test": true, "http_port": 1234});
        save_session(&tmp.to_string_lossy(), &session)
            .await
            .unwrap();

        let path = tmp.join("session.json");
        assert!(path.exists());

        let data: serde_json::Value =
            serde_json::from_str(&tokio::fs::read_to_string(&path).await.unwrap()).unwrap();
        assert_eq!(data["http_port"], 1234);

        remove_session(&tmp.to_string_lossy()).await.unwrap();
        assert!(!path.exists());

        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }

    #[test]
    fn chrono_like_now_format() {
        let s = chrono_like_now();
        assert!(s.ends_with('s'));
        let num = s.trim_end_matches('s');
        assert!(num.parse::<u64>().is_ok());
    }
}
