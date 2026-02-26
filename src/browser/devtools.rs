use anyhow::{Context, Result};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct VersionInfo {
    #[serde(rename = "webSocketDebuggerUrl")]
    web_socket_debugger_url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct DevToolsTarget {
    pub id: String,
    #[serde(rename = "type")]
    pub target_type: String,
    pub url: String,
    #[serde(rename = "webSocketDebuggerUrl", default)]
    pub web_socket_debugger_url: String,
}

/// Poll the DevTools HTTP endpoint until we get the browser-level WebSocket URL.
pub async fn devtools_ws_url(port: u16) -> Result<String> {
    let url = format!("http://127.0.0.1:{}/json/version", port);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);

    loop {
        if let Ok(body) = reqwest_like_get(&url).await {
            if let Ok(v) = serde_json::from_str::<VersionInfo>(&body) {
                if !v.web_socket_debugger_url.is_empty() {
                    // Chrome sometimes returns a WS URL without the port
                    // (e.g. ws://127.0.0.1/devtools/...) even though DevTools
                    // is on a non-standard port.  Fix it up.
                    let ws_url = fix_ws_port(&v.web_socket_debugger_url, port);
                    return Ok(ws_url);
                }
            }
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!(
                "timed out waiting for DevTools WebSocket URL on port {}",
                port
            );
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
}

/// Ensure the WebSocket URL contains the correct port.
///
/// Chrome may return `ws://127.0.0.1/devtools/...` (port 80 implied) when
/// the actual DevTools listener is on a different port.  We detect this by
/// checking if the URL's authority section has an explicit port; if not, we
/// insert the expected one.
pub(crate) fn fix_ws_port(ws_url: &str, expected_port: u16) -> String {
    // Fast path: URL already contains :<port>.
    // Pattern: ws://HOST:PORT/path  — after "ws://" or "wss://" look for ':'
    let scheme_end = if ws_url.starts_with("wss://") {
        6
    } else if ws_url.starts_with("ws://") {
        5
    } else {
        return ws_url.to_string();
    };
    let rest = &ws_url[scheme_end..]; // "HOST:PORT/path" or "HOST/path"
    let slash_pos = rest.find('/').unwrap_or(rest.len());
    let authority = &rest[..slash_pos];

    if authority.contains(':') {
        // Port already present — use as-is.
        return ws_url.to_string();
    }

    // No port in the authority — splice in the expected port.
    format!(
        "{}{}:{}/{}",
        &ws_url[..scheme_end],
        authority,
        expected_port,
        &rest[slash_pos..].trim_start_matches('/')
    )
}

/// Get the list of DevTools targets.
pub async fn devtools_targets(port: u16) -> Result<Vec<DevToolsTarget>> {
    let url = format!("http://127.0.0.1:{}/json/list", port);
    let body = reqwest_like_get(&url)
        .await
        .context("failed to get DevTools targets")?;
    let targets: Vec<DevToolsTarget> = serde_json::from_str(&body)?;
    Ok(targets)
}

/// Minimal HTTP GET using hyper (no external reqwest dependency).
async fn reqwest_like_get(url: &str) -> Result<String> {
    use std::str::FromStr;

    let uri = hyper::Uri::from_str(url)?;
    let host = uri.host().unwrap_or("127.0.0.1");
    let port = uri.port_u16().unwrap_or(80);

    let stream = tokio::net::TcpStream::connect(format!("{}:{}", host, port)).await?;
    let io = hyper_util::rt::TokioIo::new(stream);

    let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;
    tokio::spawn(async move {
        let _ = conn.await;
    });

    let req = hyper::Request::builder()
        .method("GET")
        .uri(uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/"))
        .header("host", host)
        .body(http_body_util::Empty::<bytes::Bytes>::new())?;

    let resp = sender.send_request(req).await?;
    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await?
        .to_bytes();
    Ok(String::from_utf8_lossy(&body).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fix_ws_port_already_has_port() {
        let url = "ws://127.0.0.1:9222/devtools/browser/abc";
        assert_eq!(fix_ws_port(url, 38597), url);
    }

    #[test]
    fn fix_ws_port_missing_port() {
        assert_eq!(
            fix_ws_port("ws://127.0.0.1/devtools/browser/abc", 38597),
            "ws://127.0.0.1:38597/devtools/browser/abc"
        );
    }

    #[test]
    fn fix_ws_port_wss_missing_port() {
        assert_eq!(
            fix_ws_port("wss://127.0.0.1/devtools/browser/abc", 9222),
            "wss://127.0.0.1:9222/devtools/browser/abc"
        );
    }

    #[test]
    fn fix_ws_port_non_ws_scheme_unchanged() {
        let url = "http://127.0.0.1/foo";
        assert_eq!(fix_ws_port(url, 9222), url);
    }

    #[test]
    fn fix_ws_port_no_path() {
        assert_eq!(fix_ws_port("ws://127.0.0.1", 9222), "ws://127.0.0.1:9222/");
    }
}
