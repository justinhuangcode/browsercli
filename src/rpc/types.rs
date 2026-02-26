use serde::{Deserialize, Serialize};

/// Schema version for CLI JSON output.
/// Bump major for breaking changes (removed/renamed fields).
/// Bump minor for additive changes (new fields).
pub const SCHEMA_VERSION: u32 = 1;

/// RPC protocol version.
/// Compatibility policy:
/// - New fields with `#[serde(default)]` are non-breaking (minor bump).
/// - Removing/renaming fields is breaking (major bump).
pub const RPC_VERSION: u32 = 1;

// --- Status ---
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct StatusResponse {
    pub running: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub browser_alive: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pid: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dir: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub http_addr: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub http_port: u16,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub current_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub headless: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub browser_pid: u32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub devtools_port: u16,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub devtools_ws_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub browser_bin: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error: String,
}

fn is_zero<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

// --- Goto ---
#[derive(Debug, Serialize, Deserialize)]
pub struct GotoRequest {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GotoResponse {
    pub url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title: String,
}

// --- Eval ---
#[derive(Debug, Serialize, Deserialize)]
pub struct EvalRequest {
    pub expression: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EvalResponse {
    pub value: serde_json::Value,
}

// --- Reload ---
#[derive(Debug, Serialize, Deserialize)]
pub struct ReloadResponse {
    pub ok: bool,
}

// --- Dom ---
#[derive(Debug, Serialize, Deserialize)]
pub struct DomRequest {
    pub selector: String,
    #[serde(default)]
    pub mode: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DomResponse {
    pub selector: String,
    pub mode: String,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DomAllRequest {
    pub selector: String,
    #[serde(default)]
    pub mode: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DomAllResponse {
    pub selector: String,
    pub mode: String,
    pub values: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DomAttrRequest {
    pub selector: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DomAttrResponse {
    pub selector: String,
    pub name: String,
    pub value: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DomClickRequest {
    pub selector: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DomClickResponse {
    pub ok: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DomTypeRequest {
    pub selector: String,
    pub text: String,
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DomTypeResponse {
    pub ok: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DomWaitRequest {
    pub selector: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub timeout_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DomWaitResponse {
    pub ok: bool,
    pub state: String,
}

// --- Screenshot ---
#[derive(Debug, Serialize, Deserialize)]
pub struct ScreenshotRequest {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub selector: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub format: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ScreenshotResponse {
    pub format: String,
    pub base64: String,
}

// --- Stop ---
#[derive(Debug, Serialize, Deserialize)]
pub struct StopResponse {
    pub ok: bool,
}

// --- Console (beyond Go version) ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsoleEntry {
    pub level: String,
    pub text: String,
    pub timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConsoleRequest {
    #[serde(default)]
    pub level: String,
    #[serde(default)]
    pub limit: usize,
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConsoleResponse {
    pub entries: Vec<ConsoleEntry>,
}

// --- Network (beyond Go version) ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEntry {
    pub method: String,
    pub url: String,
    pub status: u16,
    pub resource_type: String,
    pub mime_type: String,
    pub size: u64,
    pub duration_ms: u64,
    pub timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkRequest {
    #[serde(default)]
    pub limit: usize,
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkResponse {
    pub entries: Vec<NetworkEntry>,
}

// --- Performance (beyond Go version) ---
#[derive(Debug, Serialize, Deserialize)]
pub struct PerfResponse {
    pub dom_content_loaded_ms: f64,
    pub load_event_ms: f64,
}

// --- Plugins ---
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginListResponse {
    pub plugins: Vec<PluginInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub templates: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hooks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rpc_endpoints: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_response_default() {
        let s = StatusResponse::default();
        assert!(!s.running);
        assert_eq!(s.pid, 0);
        assert!(s.dir.is_empty());
    }

    #[test]
    fn status_response_skip_empty_fields() {
        let s = StatusResponse {
            running: true,
            pid: 1234,
            ..Default::default()
        };
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["running"], true);
        assert_eq!(json["pid"], 1234);
        assert!(json.get("dir").is_none());
        assert!(json.get("title").is_none());
        assert!(json.get("http_addr").is_none());
        assert!(json.get("devtools_port").is_none());
    }

    #[test]
    fn status_response_roundtrip() {
        let s = StatusResponse {
            running: true,
            browser_alive: true,
            pid: 999,
            dir: "/tmp/test".to_string(),
            http_addr: "127.0.0.1".to_string(),
            http_port: 8080,
            current_url: "http://localhost/".to_string(),
            title: "Test".to_string(),
            headless: false,
            browser_pid: 888,
            devtools_port: 9222,
            devtools_ws_url: "ws://127.0.0.1:9222/".to_string(),
            browser_bin: "/usr/bin/chromium".to_string(),
            error: String::new(),
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: StatusResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.pid, 999);
        assert_eq!(back.dir, "/tmp/test");
        assert_eq!(back.http_port, 8080);
    }

    #[test]
    fn goto_request_roundtrip() {
        let req = GotoRequest {
            url: "http://example.com".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: GotoRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.url, "http://example.com");
    }

    #[test]
    fn eval_response_various_values() {
        // number
        let resp = EvalResponse {
            value: serde_json::json!(42),
        };
        let back: EvalResponse =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(back.value, 42);

        // string
        let resp = EvalResponse {
            value: serde_json::json!("hello"),
        };
        let back: EvalResponse =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(back.value, "hello");

        // null
        let resp = EvalResponse {
            value: serde_json::Value::Null,
        };
        let back: EvalResponse =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert!(back.value.is_null());
    }

    #[test]
    fn dom_request_default_mode() {
        let req: DomRequest = serde_json::from_str(r##"{"selector":"#app"}"##).unwrap();
        assert_eq!(req.selector, "#app");
        assert!(req.mode.is_empty());
    }

    #[test]
    fn dom_all_response_roundtrip() {
        let resp = DomAllResponse {
            selector: "p".to_string(),
            mode: "text".to_string(),
            values: vec!["a".to_string(), "b".to_string()],
        };
        let back: DomAllResponse =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert_eq!(back.values.len(), 2);
    }

    #[test]
    fn dom_attr_response_none_value() {
        let resp = DomAttrResponse {
            selector: "img".to_string(),
            name: "alt".to_string(),
            value: None,
        };
        let back: DomAttrResponse =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert!(back.value.is_none());
    }

    #[test]
    fn dom_type_request_clear_default_false() {
        let req: DomTypeRequest =
            serde_json::from_str(r#"{"selector":"input","text":"hello"}"#).unwrap();
        assert!(!req.clear);
    }

    #[test]
    fn console_entry_clone() {
        let e = ConsoleEntry {
            level: "error".to_string(),
            text: "boom".to_string(),
            timestamp: 12345,
        };
        let e2 = e.clone();
        assert_eq!(e2.level, "error");
        assert_eq!(e2.timestamp, 12345);
    }

    #[test]
    fn network_entry_roundtrip() {
        let e = NetworkEntry {
            method: "GET".to_string(),
            url: "https://example.com".to_string(),
            status: 200,
            resource_type: "Document".to_string(),
            mime_type: "text/html".to_string(),
            size: 1024,
            duration_ms: 50,
            timestamp: 99999,
        };
        let back: NetworkEntry = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back.status, 200);
        assert_eq!(back.method, "GET");
        assert_eq!(back.resource_type, "Document");
        assert_eq!(back.size, 1024);
    }

    #[test]
    fn screenshot_request_skip_empty() {
        let req = ScreenshotRequest {
            selector: String::new(),
            format: "png".to_string(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("selector").is_none());
    }

    #[test]
    fn perf_response_roundtrip() {
        let resp = PerfResponse {
            dom_content_loaded_ms: 123.5,
            load_event_ms: 456.7,
        };
        let back: PerfResponse =
            serde_json::from_str(&serde_json::to_string(&resp).unwrap()).unwrap();
        assert!((back.dom_content_loaded_ms - 123.5).abs() < f64::EPSILON);
        assert!((back.load_event_ms - 456.7).abs() < f64::EPSILON);
    }

    #[test]
    fn is_zero_works() {
        assert!(is_zero(&0u16));
        assert!(is_zero(&0u32));
        assert!(!is_zero(&1u16));
        assert!(!is_zero(&42u32));
    }

    #[test]
    fn plugin_info_skip_empty_fields() {
        let info = PluginInfo {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            templates: vec![],
            hooks: vec![],
            rpc_endpoints: vec![],
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["name"], "test");
        assert!(json.get("description").is_none());
        assert!(json.get("templates").is_none());
        assert!(json.get("hooks").is_none());
        assert!(json.get("rpc_endpoints").is_none());
    }

    #[test]
    fn plugin_info_roundtrip() {
        let info = PluginInfo {
            name: "dashboard".to_string(),
            version: "1.0.0".to_string(),
            description: "Analytics".to_string(),
            templates: vec!["dash".to_string()],
            hooks: vec!["on_daemon_start".to_string()],
            rpc_endpoints: vec!["/x/dashboard/refresh".to_string()],
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: PluginInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "dashboard");
        assert_eq!(back.templates.len(), 1);
        assert_eq!(back.hooks.len(), 1);
        assert_eq!(back.rpc_endpoints.len(), 1);
    }

    #[test]
    fn plugin_list_response_roundtrip() {
        let resp = PluginListResponse {
            plugins: vec![PluginInfo {
                name: "p1".to_string(),
                version: "0.1.0".to_string(),
                description: String::new(),
                templates: vec![],
                hooks: vec![],
                rpc_endpoints: vec![],
            }],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: PluginListResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.plugins.len(), 1);
        assert_eq!(back.plugins[0].name, "p1");
    }
}
