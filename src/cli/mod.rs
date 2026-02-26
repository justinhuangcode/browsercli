use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "browsercli", bin_name = "browsercli")]
#[command(about = "A browser visual workspace for AI agents")]
#[command(version)]
pub struct Cli {
    /// Output JSON when supported
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start browsercli in the background (daemon)
    Start {
        /// Directory to serve (defaults to a temporary directory)
        #[arg(long)]
        dir: Option<String>,
        /// HTTP port (0 picks a random free port)
        #[arg(long, default_value_t = 0)]
        port: u16,
        /// DevTools remote debugging port (0 picks a random free port)
        #[arg(long, default_value_t = 0)]
        devtools_port: u16,
        /// Run browser headless
        #[arg(long)]
        headless: bool,
        /// Disable app mode (chromeless window)
        #[arg(long)]
        no_app: bool,
        /// Disable stealth mode
        #[arg(long)]
        no_stealth: bool,
        /// Browser window size, e.g. 1280,720
        #[arg(long, default_value = "1280,720")]
        window_size: String,
        /// Chromium/Chrome binary path (optional)
        #[arg(long)]
        browser_bin: Option<String>,
        /// Restart if already running
        #[arg(long)]
        restart: bool,
        /// Apply a plugin template to the serve directory
        #[arg(long)]
        template: Option<String>,
    },
    /// Run browsercli in the foreground
    Serve {
        /// Directory to serve (defaults to a temporary directory)
        #[arg(long)]
        dir: Option<String>,
        /// HTTP port (0 picks a random free port)
        #[arg(long, default_value_t = 0)]
        port: u16,
        /// DevTools remote debugging port
        #[arg(long, default_value_t = 0)]
        devtools_port: u16,
        /// Run browser headless
        #[arg(long)]
        headless: bool,
        /// Disable app mode
        #[arg(long)]
        no_app: bool,
        /// Disable stealth mode
        #[arg(long)]
        no_stealth: bool,
        /// Browser window size
        #[arg(long, default_value = "1280,720")]
        window_size: String,
        /// Chromium/Chrome binary path
        #[arg(long)]
        browser_bin: Option<String>,
        /// Apply a plugin template to the serve directory
        #[arg(long)]
        template: Option<String>,
    },
    /// Internal: run the daemon process (hidden)
    #[command(hide = true)]
    Daemon {
        #[arg(long)]
        state_dir: String,
        #[arg(long)]
        dir: String,
        #[arg(long, default_value_t = 0)]
        port: u16,
        #[arg(long, default_value_t = 0)]
        devtools_port: u16,
        #[arg(long)]
        headless: bool,
        #[arg(long)]
        app: bool,
        #[arg(long)]
        stealth: bool,
        #[arg(long, default_value = "1280,720")]
        window_size: String,
        #[arg(long)]
        browser_bin: Option<String>,
        #[arg(long)]
        temp_dir: bool,
        /// Apply a plugin template to the serve directory
        #[arg(long)]
        template: Option<String>,
    },
    /// Show current session status
    Status,
    /// Stop browsercli (server + controlled browser)
    #[command(alias = "close")]
    Stop,
    /// Bring the controlled browser window to the front (macOS)
    Focus,
    /// Print DevTools debugging port / websocket URL
    Devtools,
    /// Navigate the controlled tab to a path or full URL
    Goto {
        /// Path (e.g. /yolo) or full URL
        path: String,
    },
    /// Evaluate JavaScript in the controlled tab
    Eval {
        /// JavaScript expression
        expression: String,
    },
    /// Reload the controlled tab
    Reload,
    /// DOM utilities (query, all, attr, click, type, wait)
    Dom {
        #[command(subcommand)]
        action: Option<DomAction>,
        /// CSS selector (backward-compatible shorthand for `dom query`)
        #[arg(trailing_var_arg = true)]
        selector: Vec<String>,
        /// Query mode: outer_html or text
        #[arg(long, default_value = "outer_html")]
        mode: String,
    },
    /// Take a screenshot of the controlled tab
    Screenshot {
        /// CSS selector to screenshot (default: full page)
        #[arg(long)]
        selector: Option<String>,
        /// Output file path
        #[arg(long)]
        out: Option<String>,
    },
    /// View console output from the browser
    Console {
        /// Filter by level: log, warn, error, info
        #[arg(long)]
        level: Option<String>,
        /// Limit number of entries
        #[arg(long, default_value_t = 0)]
        limit: usize,
        /// Clear the console buffer after reading
        #[arg(long)]
        clear: bool,
    },
    /// View network requests log
    Network {
        /// Limit number of entries
        #[arg(long, default_value_t = 0)]
        limit: usize,
        /// Clear the network log buffer after reading
        #[arg(long)]
        clear: bool,
    },
    /// Show page performance metrics
    Perf,
    /// Manage plugins
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
}

#[derive(Subcommand)]
pub enum PluginAction {
    /// List installed plugins and their templates, hooks, and endpoints
    List,
    /// Scaffold a new plugin directory
    Init {
        /// Plugin name (alphanumeric, hyphens, underscores)
        name: String,
    },
}

#[derive(Subcommand)]
pub enum DomAction {
    /// Query a single element
    Query {
        /// CSS selector
        selector: String,
        /// Query mode: outer_html or text
        #[arg(long, default_value = "outer_html")]
        mode: String,
    },
    /// Query all matching elements
    All {
        /// CSS selector
        selector: String,
        /// Query mode: outer_html or text
        #[arg(long, default_value = "outer_html")]
        mode: String,
        /// Limit number of returned elements (0 = unlimited)
        #[arg(long, default_value_t = 0)]
        limit: usize,
    },
    /// Get an attribute value from the first matching element
    Attr {
        /// CSS selector
        selector: String,
        /// Attribute name
        name: String,
    },
    /// Click the first matching element
    Click {
        /// CSS selector
        selector: String,
    },
    /// Type into the first matching element
    Type {
        /// CSS selector
        selector: String,
        /// Text to type
        text: String,
        /// Clear element value before typing
        #[arg(long)]
        clear: bool,
    },
    /// Wait for a selector state (visible by default)
    Wait {
        /// CSS selector
        selector: String,
        /// State: visible, hidden, ready, present, gone
        #[arg(long, default_value = "visible")]
        state: String,
        /// Wait timeout (e.g. 10s)
        #[arg(long, default_value = "10s")]
        timeout: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(args)
    }

    #[test]
    fn parse_start_defaults() {
        let cli = parse(&["browsercli", "start"]).unwrap();
        match cli.command {
            Commands::Start {
                port,
                devtools_port,
                headless,
                no_app,
                no_stealth,
                ..
            } => {
                assert_eq!(port, 0);
                assert_eq!(devtools_port, 0);
                assert!(!headless);
                assert!(!no_app);
                assert!(!no_stealth);
            }
            _ => panic!("expected Start"),
        }
    }

    #[test]
    fn parse_start_with_flags() {
        let cli = parse(&[
            "browsercli",
            "start",
            "--dir",
            "/tmp/test",
            "--port",
            "8080",
            "--headless",
            "--no-app",
            "--window-size",
            "800,600",
        ])
        .unwrap();
        match cli.command {
            Commands::Start {
                dir,
                port,
                headless,
                no_app,
                window_size,
                ..
            } => {
                assert_eq!(dir.unwrap(), "/tmp/test");
                assert_eq!(port, 8080);
                assert!(headless);
                assert!(no_app);
                assert_eq!(window_size, "800,600");
            }
            _ => panic!("expected Start"),
        }
    }

    #[test]
    fn parse_status() {
        let cli = parse(&["browsercli", "status"]).unwrap();
        assert!(matches!(cli.command, Commands::Status));
    }

    #[test]
    fn parse_stop() {
        let cli = parse(&["browsercli", "stop"]).unwrap();
        assert!(matches!(cli.command, Commands::Stop));
    }

    #[test]
    fn parse_goto() {
        let cli = parse(&["browsercli", "goto", "/about"]).unwrap();
        match cli.command {
            Commands::Goto { path } => assert_eq!(path, "/about"),
            _ => panic!("expected Goto"),
        }
    }

    #[test]
    fn parse_eval() {
        let cli = parse(&["browsercli", "eval", "document.title"]).unwrap();
        match cli.command {
            Commands::Eval { expression } => assert_eq!(expression, "document.title"),
            _ => panic!("expected Eval"),
        }
    }

    #[test]
    fn parse_screenshot_with_options() {
        let cli = parse(&[
            "browsercli",
            "screenshot",
            "--selector",
            "#main",
            "--out",
            "shot.png",
        ])
        .unwrap();
        match cli.command {
            Commands::Screenshot { selector, out } => {
                assert_eq!(selector.unwrap(), "#main");
                assert_eq!(out.unwrap(), "shot.png");
            }
            _ => panic!("expected Screenshot"),
        }
    }

    #[test]
    fn parse_console_defaults() {
        let cli = parse(&["browsercli", "console"]).unwrap();
        match cli.command {
            Commands::Console {
                level,
                limit,
                clear,
            } => {
                assert!(level.is_none());
                assert_eq!(limit, 0);
                assert!(!clear);
            }
            _ => panic!("expected Console"),
        }
    }

    #[test]
    fn parse_console_with_level() {
        let cli = parse(&["browsercli", "console", "--level", "error", "--limit", "50"]).unwrap();
        match cli.command {
            Commands::Console {
                level,
                limit,
                clear,
            } => {
                assert_eq!(level.unwrap(), "error");
                assert_eq!(limit, 50);
                assert!(!clear);
            }
            _ => panic!("expected Console"),
        }
    }

    #[test]
    fn parse_console_with_clear() {
        let cli = parse(&["browsercli", "console", "--clear"]).unwrap();
        match cli.command {
            Commands::Console { clear, .. } => {
                assert!(clear);
            }
            _ => panic!("expected Console"),
        }
    }

    #[test]
    fn parse_network() {
        let cli = parse(&["browsercli", "network", "--limit", "10"]).unwrap();
        match cli.command {
            Commands::Network { limit, clear } => {
                assert_eq!(limit, 10);
                assert!(!clear);
            }
            _ => panic!("expected Network"),
        }
    }

    #[test]
    fn parse_network_with_clear() {
        let cli = parse(&["browsercli", "network", "--clear"]).unwrap();
        match cli.command {
            Commands::Network { clear, .. } => {
                assert!(clear);
            }
            _ => panic!("expected Network"),
        }
    }

    #[test]
    fn parse_perf() {
        let cli = parse(&["browsercli", "perf"]).unwrap();
        assert!(matches!(cli.command, Commands::Perf));
    }

    #[test]
    fn parse_json_global_flag() {
        let cli = parse(&["browsercli", "--json", "status"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn parse_dom_query_subcommand() {
        let cli = parse(&["browsercli", "dom", "query", "h1", "--mode", "text"]).unwrap();
        match cli.command {
            Commands::Dom {
                action: Some(DomAction::Query { selector, mode }),
                ..
            } => {
                assert_eq!(selector, "h1");
                assert_eq!(mode, "text");
            }
            _ => panic!("expected Dom Query"),
        }
    }

    #[test]
    fn parse_dom_click() {
        let cli = parse(&["browsercli", "dom", "click", "#btn"]).unwrap();
        match cli.command {
            Commands::Dom {
                action: Some(DomAction::Click { selector }),
                ..
            } => {
                assert_eq!(selector, "#btn");
            }
            _ => panic!("expected Dom Click"),
        }
    }

    #[test]
    fn parse_dom_wait_defaults() {
        let cli = parse(&["browsercli", "dom", "wait", ".loading"]).unwrap();
        match cli.command {
            Commands::Dom {
                action:
                    Some(DomAction::Wait {
                        selector,
                        state,
                        timeout,
                    }),
                ..
            } => {
                assert_eq!(selector, ".loading");
                assert_eq!(state, "visible");
                assert_eq!(timeout, "10s");
            }
            _ => panic!("expected Dom Wait"),
        }
    }

    #[test]
    fn parse_unknown_command_fails() {
        assert!(parse(&["browsercli", "nonexistent"]).is_err());
    }

    #[test]
    fn parse_missing_required_arg_fails() {
        assert!(parse(&["browsercli", "goto"]).is_err());
        assert!(parse(&["browsercli", "eval"]).is_err());
    }

    #[test]
    fn parse_start_with_template() {
        let cli = parse(&["browsercli", "start", "--template", "dashboard"]).unwrap();
        match cli.command {
            Commands::Start { template, .. } => {
                assert_eq!(template.unwrap(), "dashboard");
            }
            _ => panic!("expected Start"),
        }
    }

    #[test]
    fn parse_start_without_template() {
        let cli = parse(&["browsercli", "start"]).unwrap();
        match cli.command {
            Commands::Start { template, .. } => {
                assert!(template.is_none());
            }
            _ => panic!("expected Start"),
        }
    }

    #[test]
    fn parse_plugin_list() {
        let cli = parse(&["browsercli", "plugin", "list"]).unwrap();
        match cli.command {
            Commands::Plugin {
                action: PluginAction::List,
            } => {}
            _ => panic!("expected Plugin List"),
        }
    }

    #[test]
    fn parse_plugin_init() {
        let cli = parse(&["browsercli", "plugin", "init", "my-plugin"]).unwrap();
        match cli.command {
            Commands::Plugin {
                action: PluginAction::Init { name },
            } => {
                assert_eq!(name, "my-plugin");
            }
            _ => panic!("expected Plugin Init"),
        }
    }

    #[test]
    fn parse_plugin_init_missing_name_fails() {
        assert!(parse(&["browsercli", "plugin", "init"]).is_err());
    }
}
