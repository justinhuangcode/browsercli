use anyhow::{Context, Result};
use hyper::body::Bytes;
use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;
#[cfg(windows)]
use tokio::net::TcpStream;
#[cfg(unix)]
use tokio::net::UnixStream;

pub struct RpcClient {
    #[cfg(unix)]
    socket_path: String,
    #[cfg(windows)]
    rpc_addr: String,
    token: String,
}

impl RpcClient {
    #[cfg(unix)]
    pub fn new(socket_path: &str, token: &str) -> Self {
        Self {
            socket_path: socket_path.to_string(),
            token: token.to_string(),
        }
    }

    #[cfg(windows)]
    pub fn new(rpc_addr: &str, token: &str) -> Self {
        Self {
            rpc_addr: rpc_addr.to_string(),
            token: token.to_string(),
        }
    }

    async fn do_json<Req: Serialize, Resp: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<&Req>,
    ) -> Result<Resp> {
        #[cfg(unix)]
        let stream = UnixStream::connect(&self.socket_path)
            .await
            .context("daemon not running (use `browsercli start` first)")?;
        #[cfg(windows)]
        let stream = TcpStream::connect(&self.rpc_addr)
            .await
            .context("daemon not running (use `browsercli start` first)")?;

        let io = hyper_util::rt::TokioIo::new(stream);

        let (mut sender, conn) = hyper::client::conn::http1::handshake(io)
            .await
            .context("handshake failed")?;

        tokio::spawn(async move {
            let _ = conn.await;
        });

        let body_bytes = match body {
            Some(b) => serde_json::to_vec(b)?,
            None => vec![],
        };

        let req = hyper::Request::builder()
            .method(method)
            .uri(path)
            .header("authorization", format!("Bearer {}", self.token))
            .header("content-type", "application/json")
            .body(http_body_util::Full::new(Bytes::from(body_bytes)))?;

        let resp = sender.send_request(req).await?;
        let status = resp.status();

        let body_bytes = http_body_util::BodyExt::collect(resp.into_body())
            .await?
            .to_bytes();

        if !status.is_success() {
            let msg = String::from_utf8_lossy(&body_bytes);
            anyhow::bail!("{} {} failed: {} {}", method, path, status, msg.trim());
        }

        let out: Resp = serde_json::from_slice(&body_bytes)
            .with_context(|| format!("failed to decode response from {}", path))?;
        Ok(out)
    }

    pub async fn status(&self) -> Result<super::StatusResponse> {
        self.do_json::<(), _>("GET", "/status", None).await
    }

    pub async fn goto(&self, url: &str) -> Result<super::GotoResponse> {
        self.do_json(
            "POST",
            "/goto",
            Some(&super::GotoRequest {
                url: url.to_string(),
            }),
        )
        .await
    }

    pub async fn eval(&self, expression: &str) -> Result<super::EvalResponse> {
        self.do_json(
            "POST",
            "/eval",
            Some(&super::EvalRequest {
                expression: expression.to_string(),
            }),
        )
        .await
    }

    pub async fn reload(&self) -> Result<super::ReloadResponse> {
        self.do_json::<(), _>("POST", "/reload", None).await
    }

    pub async fn dom(&self, selector: &str, mode: &str) -> Result<super::DomResponse> {
        self.do_json(
            "POST",
            "/dom",
            Some(&super::DomRequest {
                selector: selector.to_string(),
                mode: mode.to_string(),
            }),
        )
        .await
    }

    pub async fn dom_all(&self, selector: &str, mode: &str) -> Result<super::DomAllResponse> {
        self.do_json(
            "POST",
            "/dom/all",
            Some(&super::DomAllRequest {
                selector: selector.to_string(),
                mode: mode.to_string(),
            }),
        )
        .await
    }

    pub async fn dom_attr(&self, selector: &str, name: &str) -> Result<super::DomAttrResponse> {
        self.do_json(
            "POST",
            "/dom/attr",
            Some(&super::DomAttrRequest {
                selector: selector.to_string(),
                name: name.to_string(),
            }),
        )
        .await
    }

    pub async fn dom_click(&self, selector: &str) -> Result<super::DomClickResponse> {
        self.do_json(
            "POST",
            "/dom/click",
            Some(&super::DomClickRequest {
                selector: selector.to_string(),
            }),
        )
        .await
    }

    pub async fn dom_type(
        &self,
        selector: &str,
        text: &str,
        clear: bool,
    ) -> Result<super::DomTypeResponse> {
        self.do_json(
            "POST",
            "/dom/type",
            Some(&super::DomTypeRequest {
                selector: selector.to_string(),
                text: text.to_string(),
                clear,
            }),
        )
        .await
    }

    pub async fn dom_wait(
        &self,
        selector: &str,
        state: &str,
        timeout_ms: u64,
    ) -> Result<super::DomWaitResponse> {
        self.do_json(
            "POST",
            "/dom/wait",
            Some(&super::DomWaitRequest {
                selector: selector.to_string(),
                state: state.to_string(),
                timeout_ms,
            }),
        )
        .await
    }

    pub async fn screenshot(&self, selector: &str) -> Result<super::ScreenshotResponse> {
        self.do_json(
            "POST",
            "/screenshot",
            Some(&super::ScreenshotRequest {
                selector: selector.to_string(),
                format: "png".to_string(),
            }),
        )
        .await
    }

    pub async fn stop(&self) -> Result<super::StopResponse> {
        self.do_json::<(), _>("POST", "/stop", None).await
    }

    pub async fn console(
        &self,
        level: &str,
        limit: usize,
        clear: bool,
    ) -> Result<super::ConsoleResponse> {
        self.do_json(
            "POST",
            "/console",
            Some(&super::ConsoleRequest {
                level: level.to_string(),
                limit,
                clear,
            }),
        )
        .await
    }

    pub async fn network(&self, limit: usize, clear: bool) -> Result<super::NetworkResponse> {
        self.do_json(
            "POST",
            "/network",
            Some(&super::NetworkRequest { limit, clear }),
        )
        .await
    }

    pub async fn perf(&self) -> Result<super::PerfResponse> {
        self.do_json::<(), _>("GET", "/perf", None).await
    }

    #[allow(dead_code)]
    pub async fn plugin_list(&self) -> Result<super::PluginListResponse> {
        self.do_json::<(), _>("GET", "/plugins", None).await
    }
}

#[allow(dead_code)]
pub fn load_session() -> Result<(String, String, String)> {
    let state_dir = super::super::state_dir()?;
    let session_path = Path::new(&state_dir).join("session.json");
    let data = std::fs::read_to_string(&session_path)
        .context("no running session (use `browsercli start` first)")?;
    let sess: serde_json::Value = serde_json::from_str(&data)?;
    let token = sess["token"].as_str().unwrap_or("").to_string();

    #[cfg(unix)]
    let addr = sess["socket_path"].as_str().unwrap_or("").to_string();
    #[cfg(windows)]
    let addr = {
        let port = sess["rpc_port"].as_u64().unwrap_or(0);
        format!("127.0.0.1:{}", port)
    };

    Ok((addr, token, state_dir))
}

pub fn must_client() -> Result<(RpcClient, serde_json::Value, String)> {
    let state_dir = super::super::state_dir()?;
    let session_path = Path::new(&state_dir).join("session.json");
    let data = std::fs::read_to_string(&session_path)
        .context("no running session (use `browsercli start` first)")?;
    let sess: serde_json::Value = serde_json::from_str(&data)?;
    let token = sess["token"].as_str().unwrap_or("").to_string();

    #[cfg(unix)]
    let addr = sess["socket_path"].as_str().unwrap_or("").to_string();
    #[cfg(windows)]
    let addr = {
        let port = sess["rpc_port"].as_u64().unwrap_or(0);
        format!("127.0.0.1:{}", port)
    };

    Ok((RpcClient::new(&addr, &token), sess, state_dir))
}
