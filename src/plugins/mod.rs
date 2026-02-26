//! Plugin system for browsercli.
//!
//! Plugins are directories in `~/.browsercli/plugins/<name>/` containing a
//! `plugin.json` manifest and executable scripts.  Three extension points:
//!
//! 1. **Templates** — HTML/CSS/JS scaffolds copied to the serve dir at startup.
//! 2. **Custom RPC endpoints** — scripts at `/x/<plugin>/<action>` paths.
//! 3. **Lifecycle hooks** — fire-and-forget scripts triggered by daemon events.

pub mod executor;
pub mod hooks;
pub mod registry;
pub mod templates;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// Re-exports for convenience.
pub use executor::{ExecutionContext, ScriptExecutor};
pub use hooks::{dispatch_hook, HookEvent};
pub use registry::PluginRegistry;
pub use templates::apply_template;

// ---------------------------------------------------------------------------
// Manifest types
// ---------------------------------------------------------------------------

/// Parsed `plugin.json` manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub templates: HashMap<String, TemplateEntry>,
    #[serde(default)]
    pub hooks: HashMap<String, String>,
    #[serde(default)]
    pub rpc: RpcConfig,
}

/// A single template entry in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateEntry {
    #[serde(default)]
    pub description: String,
    pub source: String,
    #[serde(default = "default_entrypoint")]
    pub entrypoint: String,
}

fn default_entrypoint() -> String {
    "index.html".to_string()
}

/// RPC configuration block.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RpcConfig {
    #[serde(default)]
    pub endpoints: Vec<RpcEndpoint>,
}

/// A single custom RPC endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcEndpoint {
    pub path: String,
    pub handler: String,
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default)]
    pub description: String,
}

fn default_method() -> String {
    "POST".to_string()
}

/// A plugin that has been loaded from disk.
#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub base_dir: PathBuf,
}

// ---------------------------------------------------------------------------
// Loading & validation
// ---------------------------------------------------------------------------

/// Load a plugin from a directory containing `plugin.json`.
pub fn load_plugin(plugin_dir: &Path) -> Result<LoadedPlugin> {
    let manifest_path = plugin_dir.join("plugin.json");
    let data = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("plugin manifest not found: {}", manifest_path.display()))?;
    let manifest: PluginManifest = serde_json::from_str(&data)
        .with_context(|| format!("invalid plugin manifest at {}", manifest_path.display()))?;

    validate_manifest(&manifest, plugin_dir)?;

    Ok(LoadedPlugin {
        manifest,
        base_dir: plugin_dir.to_path_buf(),
    })
}

/// Discover all plugins in the plugins root directory.
pub fn discover_plugins(plugins_root: &Path) -> Result<Vec<LoadedPlugin>> {
    if !plugins_root.is_dir() {
        return Ok(vec![]);
    }

    let mut plugins = Vec::new();
    let entries = std::fs::read_dir(plugins_root)
        .with_context(|| format!("cannot read plugins directory: {}", plugins_root.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if !path.join("plugin.json").exists() {
            continue;
        }
        match load_plugin(&path) {
            Ok(p) => {
                tracing::info!(plugin = %p.manifest.name, version = %p.manifest.version, "loaded plugin");
                plugins.push(p);
            }
            Err(e) => {
                tracing::warn!(dir = %path.display(), error = %e, "skipping invalid plugin");
            }
        }
    }

    Ok(plugins)
}

/// Validate a parsed manifest.
fn validate_manifest(manifest: &PluginManifest, plugin_dir: &Path) -> Result<()> {
    // Name: alphanumeric + hyphens/underscores, 1-64 chars.
    if manifest.name.is_empty()
        || manifest.name.len() > 64
        || !manifest
            .name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        anyhow::bail!(
            "invalid plugin name '{}': must be 1-64 alphanumeric/hyphen/underscore characters",
            manifest.name
        );
    }

    // Version: X.Y.Z
    let parts: Vec<&str> = manifest.version.split('.').collect();
    if parts.len() != 3 || !parts.iter().all(|p| p.parse::<u32>().is_ok()) {
        anyhow::bail!(
            "invalid plugin version '{}': must be X.Y.Z",
            manifest.version
        );
    }

    // Script paths cannot escape plugin directory.
    for script in manifest.hooks.values() {
        validate_script_path(script, plugin_dir)?;
    }
    for ep in &manifest.rpc.endpoints {
        validate_script_path(&ep.handler, plugin_dir)?;
        if !ep.path.starts_with("/x/") {
            anyhow::bail!("RPC endpoint path '{}' must start with /x/", ep.path);
        }
    }

    // Template source dirs must be within plugin dir.
    for entry in manifest.templates.values() {
        let source = Path::new(&entry.source);
        if source.is_absolute() || entry.source.contains("..") {
            anyhow::bail!(
                "template source '{}' must be a relative path without '..'",
                entry.source
            );
        }
    }

    Ok(())
}

/// Ensure a script path is relative and doesn't escape the plugin directory.
fn validate_script_path(script: &str, _plugin_dir: &Path) -> Result<()> {
    let p = Path::new(script);
    if p.is_absolute() {
        anyhow::bail!("script path '{}' must be relative", script);
    }
    if script.contains("..") {
        anyhow::bail!(
            "script path '{}' must not contain '..' (directory traversal)",
            script
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ac-plugin-test-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn load_valid_plugin() {
        let dir = tmp_dir("valid");
        let plugin_dir = dir.join("my-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.json"),
            r#"{
                "name": "my-plugin",
                "version": "1.0.0",
                "description": "Test plugin",
                "templates": {},
                "hooks": {},
                "rpc": { "endpoints": [] }
            }"#,
        )
        .unwrap();

        let plugin = load_plugin(&plugin_dir).unwrap();
        assert_eq!(plugin.manifest.name, "my-plugin");
        assert_eq!(plugin.manifest.version, "1.0.0");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_minimal_manifest() {
        let dir = tmp_dir("minimal");
        let plugin_dir = dir.join("minimal");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name": "minimal", "version": "0.1.0"}"#,
        )
        .unwrap();

        let plugin = load_plugin(&plugin_dir).unwrap();
        assert_eq!(plugin.manifest.name, "minimal");
        assert!(plugin.manifest.templates.is_empty());
        assert!(plugin.manifest.hooks.is_empty());
        assert!(plugin.manifest.rpc.endpoints.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reject_invalid_name() {
        let dir = tmp_dir("badname");
        let plugin_dir = dir.join("bad");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name": "has spaces!", "version": "1.0.0"}"#,
        )
        .unwrap();

        let err = load_plugin(&plugin_dir).unwrap_err();
        assert!(err.to_string().contains("invalid plugin name"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reject_empty_name() {
        let dir = tmp_dir("emptyname");
        let plugin_dir = dir.join("empty");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name": "", "version": "1.0.0"}"#,
        )
        .unwrap();

        let err = load_plugin(&plugin_dir).unwrap_err();
        assert!(err.to_string().contains("invalid plugin name"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reject_invalid_version() {
        let dir = tmp_dir("badver");
        let plugin_dir = dir.join("badver");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name": "test", "version": "abc"}"#,
        )
        .unwrap();

        let err = load_plugin(&plugin_dir).unwrap_err();
        assert!(err.to_string().contains("invalid plugin version"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reject_script_path_traversal() {
        let dir = tmp_dir("traversal");
        let plugin_dir = dir.join("traversal");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name": "test", "version": "1.0.0", "hooks": {"on_daemon_start": "../../../etc/passwd"}}"#,
        )
        .unwrap();

        let err = load_plugin(&plugin_dir).unwrap_err();
        assert!(err.to_string().contains(".."));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reject_absolute_script_path() {
        let dir = tmp_dir("abspath");
        let plugin_dir = dir.join("abspath");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name": "test", "version": "1.0.0", "hooks": {"on_daemon_start": "/bin/bash"}}"#,
        )
        .unwrap();

        let err = load_plugin(&plugin_dir).unwrap_err();
        assert!(err.to_string().contains("must be relative"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reject_rpc_path_without_x_prefix() {
        let dir = tmp_dir("noxprefix");
        let plugin_dir = dir.join("nox");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name": "test", "version": "1.0.0", "rpc": {"endpoints": [{"path": "/status", "handler": "h.sh"}]}}"#,
        )
        .unwrap();

        let err = load_plugin(&plugin_dir).unwrap_err();
        assert!(err.to_string().contains("/x/"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reject_missing_manifest() {
        let dir = tmp_dir("nomani");
        let plugin_dir = dir.join("nomani");
        fs::create_dir_all(&plugin_dir).unwrap();

        let err = load_plugin(&plugin_dir).unwrap_err();
        assert!(err.to_string().contains("plugin manifest not found"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn reject_invalid_json() {
        let dir = tmp_dir("badjson");
        let plugin_dir = dir.join("badjson");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(plugin_dir.join("plugin.json"), "not valid json {{{").unwrap();

        let err = load_plugin(&plugin_dir).unwrap_err();
        assert!(err.to_string().contains("invalid plugin manifest"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_multiple_plugins() {
        let dir = tmp_dir("discover");
        let p1 = dir.join("alpha");
        let p2 = dir.join("beta");
        fs::create_dir_all(&p1).unwrap();
        fs::create_dir_all(&p2).unwrap();
        fs::write(
            p1.join("plugin.json"),
            r#"{"name": "alpha", "version": "1.0.0"}"#,
        )
        .unwrap();
        fs::write(
            p2.join("plugin.json"),
            r#"{"name": "beta", "version": "2.0.0"}"#,
        )
        .unwrap();

        let plugins = discover_plugins(&dir).unwrap();
        assert_eq!(plugins.len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_skips_non_plugin_dirs() {
        let dir = tmp_dir("skip");
        // Directory without plugin.json.
        let empty = dir.join("empty-dir");
        fs::create_dir_all(&empty).unwrap();
        // Regular file (not a directory).
        fs::write(dir.join("not-a-dir.txt"), "hi").unwrap();
        // Valid plugin.
        let p = dir.join("valid");
        fs::create_dir_all(&p).unwrap();
        fs::write(
            p.join("plugin.json"),
            r#"{"name": "valid", "version": "1.0.0"}"#,
        )
        .unwrap();

        let plugins = discover_plugins(&dir).unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].manifest.name, "valid");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn discover_nonexistent_dir_returns_empty() {
        let dir = std::env::temp_dir().join("ac-plugin-test-noexist-999");
        let _ = std::fs::remove_dir_all(&dir);
        let plugins = discover_plugins(&dir).unwrap();
        assert!(plugins.is_empty());
    }

    #[test]
    fn template_source_traversal_rejected() {
        let dir = tmp_dir("tmpltraversal");
        let plugin_dir = dir.join("tmpl");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(
            plugin_dir.join("plugin.json"),
            r#"{"name": "test", "version": "1.0.0", "templates": {"bad": {"source": "../../secret/", "entrypoint": "index.html"}}}"#,
        )
        .unwrap();

        let err = load_plugin(&plugin_dir).unwrap_err();
        assert!(err.to_string().contains(".."));
        let _ = fs::remove_dir_all(&dir);
    }
}
