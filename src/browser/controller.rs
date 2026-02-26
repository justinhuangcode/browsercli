use anyhow::{Context, Result};
use std::collections::VecDeque;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use super::devtools;
use crate::rpc::types::{ConsoleEntry, NetworkEntry};

#[allow(dead_code)]
pub struct BrowserController {
    child: Mutex<Option<Child>>,
    browser_bin: String,
    headless: bool,
    browser_pid: u32,
    devtools_port: u16,
    devtools_ws_url: String,
    ws_conn: Mutex<
        Option<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
        >,
    >,
    target_id: Mutex<String>,
    cmd_id: Mutex<u64>,
    console_buffer: Arc<Mutex<VecDeque<ConsoleEntry>>>,
    network_buffer: Arc<Mutex<VecDeque<NetworkEntry>>>,
    /// Maps CDP requestId -> (HTTP method, start timestamp in ms) from requestWillBeSent.
    request_info: Arc<Mutex<std::collections::HashMap<String, (String, u64)>>>,
}

/// Options for launching the browser.
pub struct LaunchOptions {
    pub browser_bin: String,
    pub headless: bool,
    pub user_data_dir: String,
    pub devtools_port: u16,
    pub start_url: String,
    pub app_mode: bool,
    pub window_size: String,
    pub stealth: bool,
}

impl BrowserController {
    pub async fn launch(opts: LaunchOptions) -> Result<Arc<Self>> {
        let args = build_launch_args(&opts);

        let debug = std::env::var("BROWSERCLI_DEBUG").is_ok();
        let mut cmd = Command::new(&opts.browser_bin);
        cmd.args(&args);
        if debug {
            cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        } else {
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
        }
        #[cfg(unix)]
        {
            cmd.process_group(0);
        }
        #[cfg(windows)]
        {
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
            cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
        }

        let child = cmd.spawn().context("failed to launch browser")?;
        let browser_pid = child.id().unwrap_or(0);

        tracing::info!(
            browser_bin = %opts.browser_bin,
            pid = browser_pid,
            devtools_port = opts.devtools_port,
            "browser spawned"
        );

        // Wait for DevTools to be available.
        let ws_url = devtools::devtools_ws_url(opts.devtools_port)
            .await
            .with_context(|| {
                format!(
                    "waiting for DevTools on port {} (browser={})",
                    opts.devtools_port, opts.browser_bin
                )
            })?;

        tracing::info!(ws_url = %ws_url, "DevTools WebSocket URL obtained");

        // Connect via WebSocket to the browser's DevTools.
        // Retry a few times in case the WS endpoint isn't fully ready yet.
        let mut ws_result = None;
        for attempt in 0..5 {
            match tokio_tungstenite::connect_async(&ws_url).await {
                Ok(pair) => {
                    ws_result = Some(pair);
                    break;
                }
                Err(e) => {
                    if attempt < 4 {
                        tracing::debug!(attempt, error = %e, "WS connect retry");
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    } else {
                        return Err(e).with_context(|| {
                            format!("failed to connect to DevTools WebSocket at {}", ws_url)
                        });
                    }
                }
            }
        }
        let (ws, _) = ws_result.unwrap();

        let console_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(500)));
        let network_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(500)));
        let request_info = Arc::new(Mutex::new(std::collections::HashMap::new()));

        // Connect to the *page-level* WebSocket (not the browser-level one).
        // This lets us send CDP commands directly to the page without the
        // Target.sendMessageToTarget wrapping, which is more reliable.
        let page_ws = Self::connect_to_page_ws(opts.devtools_port, &opts.start_url)
            .await
            .unwrap_or(None);

        let (final_ws, target_id) = if let Some((page_ws_stream, tid)) = page_ws {
            // We got a page-level WS; target_id is empty so cdp_send
            // sends commands directly (no wrapping).
            tracing::info!("connected to page-level WebSocket (target {})", tid);
            (page_ws_stream, String::new())
        } else {
            // Fall back to browser-level WS with target wrapping.
            tracing::info!("using browser-level WebSocket with target wrapping");
            (ws, String::new())
        };

        let ctrl = Arc::new(Self {
            child: Mutex::new(Some(child)),
            browser_bin: opts.browser_bin.clone(),
            headless: opts.headless,
            browser_pid,
            devtools_port: opts.devtools_port,
            devtools_ws_url: ws_url.clone(),
            ws_conn: Mutex::new(Some(final_ws)),
            target_id: Mutex::new(target_id),
            cmd_id: Mutex::new(1),
            console_buffer,
            network_buffer,
            request_info,
        });

        // Enable console & network capture (beyond Go version).
        tracing::info!("enabling console capture");
        ctrl.enable_console_capture().await.ok();
        tracing::info!("enabling network capture");
        ctrl.enable_network_capture().await.ok();
        tracing::info!("browser controller ready");

        if opts.stealth {
            ctrl.apply_stealth().await.ok();
        }

        Ok(ctrl)
    }

    async fn next_id(&self) -> u64 {
        let mut id = self.cmd_id.lock().await;
        let current = *id;
        *id += 1;
        current
    }

    /// Send a CDP command and return the result.
    pub async fn cdp_send(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.cdp_send_inner(method, params),
        )
        .await
        .map_err(|_| anyhow::anyhow!("CDP command timed out: {}", method))?
    }

    async fn cdp_send_inner(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        use futures_util::{SinkExt, StreamExt};
        use tokio_tungstenite::tungstenite::Message;

        let id = self.next_id().await;
        let target_id = self.target_id.lock().await.clone();

        let msg = if target_id.is_empty() {
            serde_json::json!({
                "id": id,
                "method": method,
                "params": params
            })
        } else {
            // Flatten to use Target.sendMessageToTarget for page-level commands.
            let inner = serde_json::json!({
                "id": id,
                "method": method,
                "params": params
            });
            serde_json::json!({
                "id": id,
                "method": "Target.sendMessageToTarget",
                "params": {
                    "targetId": target_id,
                    "message": inner.to_string()
                }
            })
        };

        let mut ws = self.ws_conn.lock().await;
        let ws = ws.as_mut().context("WebSocket connection closed")?;

        ws.send(Message::Text(msg.to_string())).await?;

        // Read responses until we find our id.
        loop {
            match ws.next().await {
                Some(Ok(Message::Text(text))) => {
                    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();

                    // Check if it's a Target.receivedMessageFromTarget wrapper.
                    if v["method"] == "Target.receivedMessageFromTarget" {
                        if let Some(inner_msg) = v["params"]["message"].as_str() {
                            let inner: serde_json::Value =
                                serde_json::from_str(inner_msg).unwrap_or_default();
                            if inner["id"] == id {
                                if let Some(err) = inner.get("error") {
                                    anyhow::bail!("CDP error: {}", err);
                                }
                                return Ok(inner["result"].clone());
                            }
                            // Handle async events (console, network).
                            self.handle_event(&inner).await;
                        }
                        continue;
                    }

                    // Direct response.
                    if v["id"] == id {
                        if let Some(err) = v.get("error") {
                            anyhow::bail!("CDP error: {}", err);
                        }
                        return Ok(v["result"].clone());
                    }

                    // Handle events.
                    self.handle_event(&v).await;
                }
                Some(Ok(_)) => continue,
                Some(Err(e)) => anyhow::bail!("WebSocket error: {}", e),
                None => anyhow::bail!("WebSocket closed"),
            }
        }
    }

    async fn handle_event(&self, v: &serde_json::Value) {
        let method = v["method"].as_str().unwrap_or("");
        match method {
            "Runtime.consoleAPICalled" => {
                let args = v["params"]["args"].as_array();
                let level = v["params"]["type"].as_str().unwrap_or("log").to_string();
                let text = args
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| {
                                v["value"].as_str().map(|s| s.to_string()).or_else(|| {
                                    Some(v["description"].as_str().unwrap_or("").to_string())
                                })
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_default();

                let entry = ConsoleEntry {
                    level,
                    text,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                };

                let mut buf = self.console_buffer.lock().await;
                if buf.len() >= 500 {
                    buf.pop_front();
                }
                buf.push_back(entry);
            }
            "Network.requestWillBeSent" => {
                let request_id = v["params"]["requestId"].as_str().unwrap_or("").to_string();
                let method = v["params"]["request"]["method"]
                    .as_str()
                    .unwrap_or("GET")
                    .to_string();
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                if !request_id.is_empty() {
                    let mut info = self.request_info.lock().await;
                    // Cap stored entries to prevent unbounded growth.
                    if info.len() > 2000 {
                        info.clear();
                    }
                    info.insert(request_id, (method, now));
                }
            }
            "Network.responseReceived" => {
                let request_id = v["params"]["requestId"].as_str().unwrap_or("");
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let (http_method, duration_ms) = {
                    let mut info = self.request_info.lock().await;
                    match info.remove(request_id) {
                        Some((method, start)) => (method, now.saturating_sub(start)),
                        None => (String::new(), 0),
                    }
                };
                let resp = &v["params"]["response"];
                let entry = NetworkEntry {
                    method: http_method,
                    url: resp["url"].as_str().unwrap_or("").to_string(),
                    status: resp["status"].as_u64().unwrap_or(0) as u16,
                    resource_type: v["params"]["type"].as_str().unwrap_or("").to_string(),
                    mime_type: resp["mimeType"].as_str().unwrap_or("").to_string(),
                    size: resp["encodedDataLength"].as_u64().unwrap_or(0),
                    duration_ms,
                    timestamp: now,
                };

                let mut buf = self.network_buffer.lock().await;
                if buf.len() >= 500 {
                    buf.pop_front();
                }
                buf.push_back(entry);
            }
            _ => {}
        }
    }

    /// Try to connect to a page-level WebSocket directly.
    ///
    /// Returns the WebSocket stream and the target id on success.
    async fn connect_to_page_ws(
        devtools_port: u16,
        start_url: &str,
    ) -> Result<
        Option<(
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            String,
        )>,
    > {
        let targets = devtools::devtools_targets(devtools_port)
            .await
            .unwrap_or_default();

        let target = targets
            .iter()
            .find(|t| t.target_type == "page" && t.url == start_url)
            .or_else(|| targets.iter().find(|t| t.target_type == "page"));

        let t = match target {
            Some(t) if !t.web_socket_debugger_url.is_empty() => t,
            _ => return Ok(None),
        };

        let ws_url = devtools::fix_ws_port(&t.web_socket_debugger_url, devtools_port);
        tracing::info!(ws_url = %ws_url, target_id = %t.id, "connecting to page-level WS");

        // Retry connection (same pattern as browser-level WS).
        for attempt in 0..5 {
            match tokio_tungstenite::connect_async(&ws_url).await {
                Ok((stream, _)) => return Ok(Some((stream, t.id.clone()))),
                Err(e) => {
                    if attempt < 4 {
                        tracing::debug!(attempt, error = %e, "page WS connect retry");
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    } else {
                        tracing::warn!(error = %e, "failed to connect page-level WS, will fall back");
                        return Ok(None);
                    }
                }
            }
        }
        Ok(None)
    }

    async fn enable_console_capture(&self) -> Result<()> {
        self.cdp_send("Runtime.enable", serde_json::json!({}))
            .await?;
        Ok(())
    }

    async fn enable_network_capture(&self) -> Result<()> {
        self.cdp_send("Network.enable", serde_json::json!({}))
            .await?;
        Ok(())
    }

    async fn apply_stealth(&self) -> Result<()> {
        let script = r#"(function () {
  try { Object.defineProperty(navigator, 'webdriver', { get: () => undefined }); } catch (_) {}
  try { window.chrome = window.chrome || {}; } catch (_) {}
})();"#;

        self.cdp_send(
            "Page.addScriptToEvaluateOnNewDocument",
            serde_json::json!({ "source": script }),
        )
        .await
        .ok();

        // Also run now.
        self.cdp_send(
            "Runtime.evaluate",
            serde_json::json!({ "expression": script }),
        )
        .await
        .ok();

        Ok(())
    }

    // --- Public API (matching Go version + beyond) ---

    pub fn browser_binary(&self) -> &str {
        &self.browser_bin
    }
    #[allow(dead_code)]
    pub fn is_headless(&self) -> bool {
        self.headless
    }
    pub fn browser_pid(&self) -> u32 {
        self.browser_pid
    }
    pub fn devtools_port(&self) -> u16 {
        self.devtools_port
    }
    pub fn devtools_ws_url(&self) -> &str {
        &self.devtools_ws_url
    }

    pub async fn alive(&self) -> bool {
        self.cdp_send("Runtime.evaluate", serde_json::json!({ "expression": "1" }))
            .await
            .is_ok()
    }

    pub async fn navigate(&self, url: &str) -> Result<(String, String)> {
        self.cdp_send("Page.navigate", serde_json::json!({ "url": url }))
            .await?;

        // Wait for load.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let loc = self.location().await.unwrap_or_default();
        let title = self.title().await.unwrap_or_default();
        Ok((loc, title))
    }

    pub async fn reload(&self) -> Result<()> {
        self.cdp_send("Page.reload", serde_json::json!({})).await?;
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        Ok(())
    }

    pub async fn eval(&self, expression: &str) -> Result<serde_json::Value> {
        let result = self
            .cdp_send(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true
                }),
            )
            .await?;
        Ok(result["result"]["value"].clone())
    }

    pub async fn location(&self) -> Result<String> {
        let v = self.eval("window.location.href").await?;
        Ok(v.as_str().unwrap_or("").to_string())
    }

    pub async fn title(&self) -> Result<String> {
        let v = self.eval("document.title").await?;
        Ok(v.as_str().unwrap_or("").to_string())
    }

    pub async fn outer_html(&self, selector: &str) -> Result<String> {
        let expr = format!(
            "document.querySelector({})?.outerHTML ?? ''",
            serde_json::to_string(selector)?
        );
        let v = self.eval(&expr).await?;
        Ok(v.as_str().unwrap_or("").to_string())
    }

    pub async fn text(&self, selector: &str) -> Result<String> {
        let expr = format!(
            "(document.querySelector({})?.textContent ?? '').trim()",
            serde_json::to_string(selector)?
        );
        let v = self.eval(&expr).await?;
        Ok(v.as_str().unwrap_or("").to_string())
    }

    pub async fn query_all(&self, selector: &str, mode: &str) -> Result<Vec<String>> {
        let sel = serde_json::to_string(selector)?;
        let expr = match mode {
            "text" => format!(
                "Array.from(document.querySelectorAll({})).map(n => (n.textContent ?? '').trim())",
                sel
            ),
            _ => format!(
                "Array.from(document.querySelectorAll({})).map(n => n.outerHTML)",
                sel
            ),
        };
        let v = self.eval(&expr).await?;
        let arr = v.as_array().map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });
        Ok(arr.unwrap_or_default())
    }

    pub async fn attr(&self, selector: &str, name: &str) -> Result<Option<String>> {
        let sel = serde_json::to_string(selector)?;
        let attr_name = serde_json::to_string(name)?;
        let expr = format!(
            r#"(() => {{ const el = document.querySelector({}); if (!el) return {{"__canvas":"not_found"}}; return el.getAttribute({}); }})()"#,
            sel, attr_name
        );
        let v = self.eval(&expr).await?;
        if v.is_null() {
            return Ok(None);
        }
        if let Some(obj) = v.as_object() {
            if obj.get("__canvas").is_some() {
                anyhow::bail!("element not found");
            }
        }
        Ok(Some(v.as_str().unwrap_or("").to_string()))
    }

    pub async fn click(&self, selector: &str) -> Result<()> {
        let sel = serde_json::to_string(selector)?;
        let expr = format!(
            "(() => {{ const el = document.querySelector({}); if (!el) throw new Error('element not found'); el.click(); return true; }})()",
            sel
        );
        self.eval(&expr).await?;
        Ok(())
    }

    pub async fn type_text(&self, selector: &str, text: &str, clear: bool) -> Result<()> {
        let sel = serde_json::to_string(selector)?;
        if clear {
            let clear_expr = format!("document.querySelector({}).value = ''", sel);
            self.eval(&clear_expr).await.ok();
        }

        // Focus the element, then use Input.dispatchKeyEvent for each char.
        let focus_expr = format!("document.querySelector({}).focus()", sel);
        self.eval(&focus_expr).await?;

        for ch in text.chars() {
            self.cdp_send(
                "Input.dispatchKeyEvent",
                serde_json::json!({
                    "type": "keyDown",
                    "text": ch.to_string()
                }),
            )
            .await?;
            self.cdp_send(
                "Input.dispatchKeyEvent",
                serde_json::json!({
                    "type": "keyUp",
                    "text": ch.to_string()
                }),
            )
            .await?;
        }

        Ok(())
    }

    pub async fn wait(
        &self,
        selector: &str,
        state: &str,
        timeout: std::time::Duration,
    ) -> Result<()> {
        let sel = serde_json::to_string(selector)?;
        let expr = match state {
            "visible" => format!(
                "!!document.querySelector({})?.offsetParent || !!document.querySelector({})?.getClientRects().length",
                sel, sel
            ),
            "hidden" => format!(
                "!document.querySelector({})?.offsetParent && !document.querySelector({})?.getClientRects().length",
                sel, sel
            ),
            "ready" | "present" => format!("!!document.querySelector({})", sel),
            "gone" => format!("!document.querySelector({})", sel),
            _ => anyhow::bail!("unknown wait state: {}", state),
        };

        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let result = self
                .eval(&expr)
                .await
                .unwrap_or(serde_json::Value::Bool(false));
            if result.as_bool().unwrap_or(false) {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!("timeout waiting for {} to be {}", selector, state);
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    pub async fn screenshot(&self, selector: &str) -> Result<Vec<u8>> {
        let result = if selector.is_empty() {
            self.cdp_send(
                "Page.captureScreenshot",
                serde_json::json!({ "format": "png" }),
            )
            .await?
        } else {
            // Get element bounds first, then clip.
            let sel = serde_json::to_string(selector)?;
            let bounds_expr = format!(
                r#"(() => {{ const el = document.querySelector({}); if (!el) return null; const r = el.getBoundingClientRect(); return {{x: r.x, y: r.y, width: r.width, height: r.height, scale: window.devicePixelRatio}}; }})()"#,
                sel
            );
            let bounds = self.eval(&bounds_expr).await?;
            if bounds.is_null() {
                anyhow::bail!("element not found: {}", selector);
            }
            self.cdp_send(
                "Page.captureScreenshot",
                serde_json::json!({
                    "format": "png",
                    "clip": {
                        "x": bounds["x"],
                        "y": bounds["y"],
                        "width": bounds["width"],
                        "height": bounds["height"],
                        "scale": bounds["scale"].as_f64().unwrap_or(1.0)
                    }
                }),
            )
            .await?
        };

        let b64 = result["data"].as_str().unwrap_or("");
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD.decode(b64)?;
        Ok(bytes)
    }

    // --- Beyond Go version ---

    pub async fn get_console_entries(
        &self,
        level: &str,
        limit: usize,
        clear: bool,
    ) -> Vec<ConsoleEntry> {
        let mut buf = self.console_buffer.lock().await;
        let iter: Box<dyn Iterator<Item = &ConsoleEntry>> = if level.is_empty() {
            Box::new(buf.iter())
        } else {
            Box::new(buf.iter().filter(|e| e.level == level))
        };
        let entries: Vec<_> = iter.cloned().collect();
        if clear {
            buf.clear();
        }
        if limit > 0 && entries.len() > limit {
            entries[entries.len() - limit..].to_vec()
        } else {
            entries
        }
    }

    pub async fn get_network_entries(&self, limit: usize, clear: bool) -> Vec<NetworkEntry> {
        let mut buf = self.network_buffer.lock().await;
        let entries: Vec<_> = buf.iter().cloned().collect();
        if clear {
            buf.clear();
        }
        if limit > 0 && entries.len() > limit {
            entries[entries.len() - limit..].to_vec()
        } else {
            entries
        }
    }

    pub async fn get_perf_metrics(&self) -> Result<(f64, f64)> {
        let v = self
            .eval(
                r#"(() => {
            const entries = performance.getEntriesByType('navigation');
            if (entries && entries.length > 0) {
                const n = entries[0];
                return {
                    domContentLoaded: n.domContentLoadedEventEnd,
                    load: n.loadEventEnd
                };
            }
            const t = performance.timing;
            return {
                domContentLoaded: t.domContentLoadedEventEnd - t.navigationStart,
                load: t.loadEventEnd - t.navigationStart
            };
        })()"#,
            )
            .await?;
        let dcl = v["domContentLoaded"].as_f64().unwrap_or(0.0);
        let load = v["load"].as_f64().unwrap_or(0.0);
        Ok((dcl, load))
    }

    pub async fn close(&self) -> Result<()> {
        // Close WebSocket.
        {
            let mut ws = self.ws_conn.lock().await;
            *ws = None;
        }
        // Kill browser process.
        let mut child_lock = self.child.lock().await;
        if let Some(ref mut child) = *child_lock {
            // On Unix, send SIGTERM for graceful shutdown first.
            // On Windows, no equivalent — rely on child.kill() below.
            #[cfg(unix)]
            {
                unsafe { libc::kill(child.id().unwrap_or(0) as i32, libc::SIGTERM) };
            }
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), child.wait()).await;
            let _ = child.kill().await;
        }
        *child_lock = None;
        Ok(())
    }
}

pub(crate) fn build_launch_args(opts: &LaunchOptions) -> Vec<String> {
    let mut args = vec![
        "--remote-debugging-address=127.0.0.1".to_string(),
        format!("--remote-debugging-port={}", opts.devtools_port),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-background-networking".to_string(),
        "--disable-default-apps".to_string(),
        "--disable-extensions".to_string(),
        "--disable-popup-blocking".to_string(),
    ];

    if !opts.user_data_dir.is_empty() {
        args.push(format!("--user-data-dir={}", opts.user_data_dir));
    }

    if !opts.window_size.is_empty() {
        args.push(format!("--window-size={}", opts.window_size));
    }

    if opts.headless {
        args.push("--headless=new".to_string());
        args.push("--disable-gpu".to_string());
        args.push("--no-sandbox".to_string());
        args.push("--disable-dev-shm-usage".to_string());
    }

    let start_url = if opts.start_url.is_empty() {
        "about:blank".to_string()
    } else {
        opts.start_url.clone()
    };

    if opts.app_mode && !opts.headless && !opts.start_url.is_empty() {
        args.push(format!("--app={}", opts.start_url));
    } else {
        args.push(start_url);
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_opts() -> LaunchOptions {
        LaunchOptions {
            browser_bin: "/usr/bin/chromium".to_string(),
            headless: false,
            user_data_dir: "/tmp/test-profile".to_string(),
            devtools_port: 9222,
            start_url: "http://127.0.0.1:8080/".to_string(),
            app_mode: true,
            window_size: "1280,720".to_string(),
            stealth: false,
        }
    }

    #[test]
    fn test_launch_args_basic() {
        let opts = default_opts();
        let args = build_launch_args(&opts);
        assert!(args.contains(&"--remote-debugging-port=9222".to_string()));
        assert!(args.contains(&"--no-first-run".to_string()));
        assert!(args.contains(&"--user-data-dir=/tmp/test-profile".to_string()));
        assert!(args.contains(&"--window-size=1280,720".to_string()));
    }

    #[test]
    fn test_launch_args_app_mode() {
        let opts = default_opts();
        let args = build_launch_args(&opts);
        assert!(args.contains(&"--app=http://127.0.0.1:8080/".to_string()));
        // In app mode, the URL should not be passed as a bare arg.
        assert!(!args.contains(&"http://127.0.0.1:8080/".to_string()));
    }

    #[test]
    fn test_launch_args_no_app_mode() {
        let mut opts = default_opts();
        opts.app_mode = false;
        let args = build_launch_args(&opts);
        assert!(!args.iter().any(|a| a.starts_with("--app=")));
        assert!(args.contains(&"http://127.0.0.1:8080/".to_string()));
    }

    #[test]
    fn test_launch_args_headless() {
        let mut opts = default_opts();
        opts.headless = true;
        let args = build_launch_args(&opts);
        assert!(args.contains(&"--headless=new".to_string()));
        assert!(args.contains(&"--disable-gpu".to_string()));
        assert!(args.contains(&"--no-sandbox".to_string()));
        assert!(args.contains(&"--disable-dev-shm-usage".to_string()));
        // Headless disables app mode even if app_mode is set.
        assert!(!args.iter().any(|a| a.starts_with("--app=")));
    }

    #[test]
    fn test_launch_args_headless_overrides_app() {
        let mut opts = default_opts();
        opts.headless = true;
        opts.app_mode = true;
        let args = build_launch_args(&opts);
        assert!(args.contains(&"--headless=new".to_string()));
        assert!(!args.iter().any(|a| a.starts_with("--app=")));
    }

    #[test]
    fn test_launch_args_empty_start_url() {
        let mut opts = default_opts();
        opts.start_url = String::new();
        opts.app_mode = false;
        let args = build_launch_args(&opts);
        assert!(args.contains(&"about:blank".to_string()));
    }

    #[test]
    fn test_launch_args_empty_user_data_dir() {
        let mut opts = default_opts();
        opts.user_data_dir = String::new();
        let args = build_launch_args(&opts);
        assert!(!args.iter().any(|a| a.starts_with("--user-data-dir=")));
    }

    #[test]
    fn test_launch_args_empty_window_size() {
        let mut opts = default_opts();
        opts.window_size = String::new();
        let args = build_launch_args(&opts);
        assert!(!args.iter().any(|a| a.starts_with("--window-size=")));
    }
}
