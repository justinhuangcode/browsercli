use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::welcome::{render_welcome_html, WelcomeData};

pub type WelcomeProvider = Arc<Mutex<Option<Box<dyn Fn() -> WelcomeData + Send + Sync>>>>;

pub struct StaticHandler {
    root: PathBuf,
    welcome: WelcomeProvider,
}

impl StaticHandler {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            welcome: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_welcome<F: Fn() -> WelcomeData + Send + Sync + 'static>(&self, f: F) {
        // We need to set this synchronously for daemon setup, so use try_lock or blocking.
        if let Ok(mut guard) = self.welcome.try_lock() {
            *guard = Some(Box::new(f));
        }
    }

    pub async fn handle_request(&self, req_path: &str) -> (u16, String, Vec<u8>) {
        let clean = if req_path.is_empty() { "/" } else { req_path };
        let clean = clean.trim_start_matches('/');
        let target = self.root.join(clean);
        let target = target.canonicalize().unwrap_or(target.clone());

        // Security: ensure within root.
        let root_canonical = self.root.canonicalize().unwrap_or(self.root.clone());
        if !target.starts_with(&root_canonical) && target != root_canonical {
            return (404, "text/plain".to_string(), b"not found".to_vec());
        }

        // If it's a directory, try index.html.
        if target.is_dir() {
            for name in &["index.html", "index.htm"] {
                let index = target.join(name);
                if index.is_file() {
                    return self.serve_file(&index).await;
                }
            }
            // Root with no index -> welcome page.
            if clean.is_empty() || clean == "/" {
                let guard = self.welcome.lock().await;
                if let Some(ref f) = *guard {
                    let data = f();
                    let html = render_welcome_html(&data);
                    return (
                        200,
                        "text/html; charset=utf-8".to_string(),
                        html.into_bytes(),
                    );
                }
            }
            return (404, "text/plain".to_string(), b"not found".to_vec());
        }

        if target.is_file() {
            return self.serve_file(&target).await;
        }

        (404, "text/plain".to_string(), b"not found".to_vec())
    }

    async fn serve_file(&self, path: &Path) -> (u16, String, Vec<u8>) {
        match tokio::fs::read(path).await {
            Ok(data) => {
                let mime = mime_guess::from_path(path)
                    .first_or_octet_stream()
                    .to_string();
                (200, mime, data)
            }
            Err(_) => (500, "text/plain".to_string(), b"read error".to_vec()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn setup_dir() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let tmp = std::env::temp_dir().join(format!(
            "browsercli-static-test-{}-{}",
            std::process::id(),
            n,
        ));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        tmp
    }

    #[tokio::test]
    async fn serve_existing_file() {
        let dir = setup_dir();
        fs::write(dir.join("hello.txt"), "world").unwrap();
        let handler = StaticHandler::new(&dir);
        let (status, mime, body) = handler.handle_request("/hello.txt").await;
        assert_eq!(status, 200);
        assert!(mime.contains("text/plain"));
        assert_eq!(body, b"world");
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn serve_html_mime() {
        let dir = setup_dir();
        fs::write(dir.join("page.html"), "<h1>hi</h1>").unwrap();
        let handler = StaticHandler::new(&dir);
        let (status, mime, _) = handler.handle_request("/page.html").await;
        assert_eq!(status, 200);
        assert!(mime.contains("text/html"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn serve_index_html_for_root() {
        let dir = setup_dir();
        fs::write(dir.join("index.html"), "<html>index</html>").unwrap();
        let handler = StaticHandler::new(&dir);
        let (status, mime, body) = handler.handle_request("/").await;
        assert_eq!(status, 200);
        assert!(mime.contains("text/html"));
        assert_eq!(body, b"<html>index</html>");
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn serve_index_htm_fallback() {
        let dir = setup_dir();
        fs::write(dir.join("index.htm"), "<html>htm</html>").unwrap();
        let handler = StaticHandler::new(&dir);
        let (status, _, body) = handler.handle_request("/").await;
        assert_eq!(status, 200);
        assert_eq!(body, b"<html>htm</html>");
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn not_found() {
        let dir = setup_dir();
        let handler = StaticHandler::new(&dir);
        let (status, _, _) = handler.handle_request("/nope.js").await;
        assert_eq!(status, 404);
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn path_traversal_blocked() {
        let dir = setup_dir();
        let handler = StaticHandler::new(&dir);
        let (status, _, _) = handler.handle_request("/../../../etc/passwd").await;
        assert_eq!(status, 404);
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn welcome_page_when_no_index() {
        let dir = setup_dir();
        let handler = StaticHandler::new(&dir);
        handler.set_welcome(|| WelcomeData {
            serve_dir: "/test".to_string(),
            ..Default::default()
        });
        let (status, mime, body) = handler.handle_request("/").await;
        assert_eq!(status, 200);
        assert!(mime.contains("text/html"));
        let html = String::from_utf8(body).unwrap();
        assert!(html.contains("browsercli"));
        assert!(html.contains("/test"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn subdirectory_with_index() {
        let dir = setup_dir();
        let sub = dir.join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("index.html"), "<html>sub</html>").unwrap();
        let handler = StaticHandler::new(&dir);
        let (status, _, body) = handler.handle_request("/sub/").await;
        assert_eq!(status, 200);
        assert_eq!(body, b"<html>sub</html>");
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn empty_path_as_root() {
        let dir = setup_dir();
        fs::write(dir.join("index.html"), "root").unwrap();
        let handler = StaticHandler::new(&dir);
        let (status, _, body) = handler.handle_request("").await;
        assert_eq!(status, 200);
        assert_eq!(body, b"root");
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn mime_css() {
        let dir = setup_dir();
        fs::write(dir.join("style.css"), "body {}").unwrap();
        let handler = StaticHandler::new(&dir);
        let (_, mime, _) = handler.handle_request("/style.css").await;
        assert!(mime.contains("css"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn mime_js() {
        let dir = setup_dir();
        fs::write(dir.join("app.js"), "console.log(1)").unwrap();
        let handler = StaticHandler::new(&dir);
        let (_, mime, _) = handler.handle_request("/app.js").await;
        assert!(mime.contains("javascript"));
        let _ = fs::remove_dir_all(&dir);
    }
}
