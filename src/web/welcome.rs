use std::fmt::Write;

#[derive(Default)]
pub struct WelcomeData {
    pub serve_dir: String,
    pub http_url: String,
    pub devtools_port: u16,
    pub devtools_ws_url: String,
    pub auto_reload: bool,
    pub app_mode: bool,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn render_welcome_html(d: &WelcomeData) -> String {
    let title = "browsercli";
    let subtitle = "Write an index.html to get started.";

    let devtools = if !d.devtools_ws_url.is_empty() {
        d.devtools_ws_url.clone()
    } else if d.devtools_port != 0 {
        format!("127.0.0.1:{}", d.devtools_port)
    } else {
        String::new()
    };

    let mut html = String::with_capacity(4096);
    let _ = write!(
        html,
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>{title}</title>
<style>
:root {{ color-scheme: light dark; }}
body {{ font-family: ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial; margin: 0; padding: 32px; }}
.card {{ max-width: 920px; margin: 0 auto; padding: 24px; border: 1px solid rgba(127,127,127,.25); border-radius: 14px; }}
h1 {{ margin: 0 0 6px 0; font-size: 28px; }}
p {{ margin: 0 0 14px 0; line-height: 1.5; opacity: .9 }}
code, pre {{ font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace; }}
pre {{ background: rgba(127,127,127,.12); padding: 12px 14px; border-radius: 10px; overflow: auto; }}
.grid {{ display: grid; grid-template-columns: 1fr; gap: 12px; }}
.kv {{ display: grid; grid-template-columns: 140px 1fr; gap: 10px; }}
.k {{ opacity: .75 }}
.pill {{ display: inline-block; padding: 2px 8px; border: 1px solid rgba(127,127,127,.25); border-radius: 999px; font-size: 12px; opacity: .85 }}
</style>
</head>
<body>
<div class="card">
<div class="pill">browsercli</div>
<h1>{title}</h1>
<p>{subtitle}</p>
<div class="grid">
<div class="kv"><div class="k">Serving</div><div><code>{serve_dir}</code></div></div>"#,
        title = esc(title),
        subtitle = esc(subtitle),
        serve_dir = esc(&d.serve_dir),
    );

    if !d.http_url.is_empty() {
        let _ = write!(
            html,
            r#"<div class="kv"><div class="k">URL</div><div><code>{}</code></div></div>"#,
            esc(&d.http_url)
        );
    }
    if !devtools.is_empty() {
        let _ = write!(
            html,
            r#"<div class="kv"><div class="k">DevTools</div><div><code>{}</code></div></div>"#,
            esc(&devtools)
        );
    }
    if d.auto_reload {
        let _ = write!(
            html,
            r#"<div class="kv"><div class="k">Reload</div><div>Auto-reload is on (write files to refresh)</div></div>"#
        );
    }
    if d.app_mode {
        let _ = write!(
            html,
            r#"<div class="kv"><div class="k">Window</div><div>App mode (chromeless)</div></div>"#
        );
    }

    let _ = write!(
        html,
        r#"</div>
<h2 style="margin: 18px 0 10px 0; font-size: 16px;">Create your first page</h2>
<pre><code>{example}</code></pre>
<p>Then navigate with <code>browsercli goto /</code>. DOM helpers: <code>browsercli dom all "h1" --mode text</code>.</p>
</div>
</body>
</html>"#,
        example = esc(r#"cat > index.html <<'HTML'
<!doctype html>
<html>
  <body style="font-family: system-ui; padding: 24px">
    <h1>Hello browsercli</h1>
  </body>
</html>
HTML"#),
    );

    html
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_basic() {
        assert_eq!(esc("hello"), "hello");
        assert_eq!(esc("<script>"), "&lt;script&gt;");
        assert_eq!(esc("a&b"), "a&amp;b");
        assert_eq!(esc(r#"a"b"#), "a&quot;b");
    }

    #[test]
    fn esc_all_special_at_once() {
        assert_eq!(
            esc(r#"<a href="x">&</a>"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;&lt;/a&gt;"
        );
    }

    #[test]
    fn esc_empty() {
        assert_eq!(esc(""), "");
    }

    #[test]
    fn render_welcome_default() {
        let html = render_welcome_html(&WelcomeData::default());
        assert!(html.contains("<!doctype html>"));
        assert!(html.contains("browsercli"));
        assert!(html.contains("Write an index.html to get started."));
    }

    #[test]
    fn render_welcome_with_serve_dir() {
        let d = WelcomeData {
            serve_dir: "/tmp/my-project".to_string(),
            ..Default::default()
        };
        let html = render_welcome_html(&d);
        assert!(html.contains("/tmp/my-project"));
    }

    #[test]
    fn render_welcome_with_http_url() {
        let d = WelcomeData {
            http_url: "http://127.0.0.1:9999/".to_string(),
            ..Default::default()
        };
        assert!(render_welcome_html(&d).contains("http://127.0.0.1:9999/"));
    }

    #[test]
    fn render_welcome_devtools_ws_url() {
        let d = WelcomeData {
            devtools_ws_url: "ws://127.0.0.1:9222/devtools".to_string(),
            ..Default::default()
        };
        assert!(render_welcome_html(&d).contains("ws://127.0.0.1:9222/devtools"));
    }

    #[test]
    fn render_welcome_devtools_port_fallback() {
        let d = WelcomeData {
            devtools_port: 9222,
            ..Default::default()
        };
        assert!(render_welcome_html(&d).contains("127.0.0.1:9222"));
    }

    #[test]
    fn render_welcome_auto_reload() {
        let on = WelcomeData {
            auto_reload: true,
            ..Default::default()
        };
        assert!(render_welcome_html(&on).contains("Auto-reload is on"));
        let off = WelcomeData::default();
        assert!(!render_welcome_html(&off).contains("Auto-reload"));
    }

    #[test]
    fn render_welcome_app_mode() {
        let d = WelcomeData {
            app_mode: true,
            ..Default::default()
        };
        assert!(render_welcome_html(&d).contains("App mode (chromeless)"));
    }

    #[test]
    fn render_welcome_xss_safe() {
        let d = WelcomeData {
            serve_dir: "<script>alert(1)</script>".to_string(),
            ..Default::default()
        };
        let html = render_welcome_html(&d);
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;"));
    }
}
