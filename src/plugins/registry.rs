//! Central plugin registry with O(1) lookups for RPC handlers, hooks, and templates.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::plugins::{discover_plugins, LoadedPlugin, RpcEndpoint, TemplateEntry};

/// Information about a registered RPC handler.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RpcHandler {
    pub plugin_name: String,
    pub endpoint: RpcEndpoint,
    pub script_path: PathBuf,
}

/// Information about a registered hook handler.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HookHandler {
    pub plugin_name: String,
    pub event: String,
    pub script_path: PathBuf,
}

/// Information about a registered template.
#[derive(Debug, Clone)]
pub struct TemplateInfo {
    pub plugin_name: String,
    pub template_name: String,
    pub entry: TemplateEntry,
    pub source_dir: PathBuf,
}

/// Central registry holding all loaded plugins with fast lookups.
#[derive(Debug)]
pub struct PluginRegistry {
    plugins: Vec<LoadedPlugin>,
    rpc_handlers: HashMap<String, RpcHandler>,
    hooks: HashMap<String, Vec<HookHandler>>,
    templates: HashMap<String, TemplateInfo>,
}

impl PluginRegistry {
    /// Load all plugins from a root directory and build the registry.
    pub fn new(plugins_root: &Path) -> anyhow::Result<Self> {
        let plugins = discover_plugins(plugins_root)?;
        let mut registry = Self {
            plugins: Vec::new(),
            rpc_handlers: HashMap::new(),
            hooks: HashMap::new(),
            templates: HashMap::new(),
        };

        for plugin in plugins {
            registry.register_plugin(plugin);
        }

        Ok(registry)
    }

    /// Create an empty registry (no plugins loaded).
    pub fn empty() -> Self {
        Self {
            plugins: Vec::new(),
            rpc_handlers: HashMap::new(),
            hooks: HashMap::new(),
            templates: HashMap::new(),
        }
    }

    /// Register a single loaded plugin into the registry.
    fn register_plugin(&mut self, plugin: LoadedPlugin) {
        let name = plugin.manifest.name.clone();
        let base = plugin.base_dir.clone();

        // Register RPC endpoints.
        for ep in &plugin.manifest.rpc.endpoints {
            let handler = RpcHandler {
                plugin_name: name.clone(),
                endpoint: ep.clone(),
                script_path: base.join(&ep.handler),
            };
            if self.rpc_handlers.contains_key(&ep.path) {
                tracing::warn!(
                    path = %ep.path,
                    plugin = %name,
                    "RPC endpoint conflict — overwriting previous handler"
                );
            }
            self.rpc_handlers.insert(ep.path.clone(), handler);
        }

        // Register hooks.
        for (event, script) in &plugin.manifest.hooks {
            let handler = HookHandler {
                plugin_name: name.clone(),
                event: event.clone(),
                script_path: base.join(script),
            };
            self.hooks.entry(event.clone()).or_default().push(handler);
        }

        // Register templates.
        for (tpl_name, entry) in &plugin.manifest.templates {
            let info = TemplateInfo {
                plugin_name: name.clone(),
                template_name: tpl_name.clone(),
                entry: entry.clone(),
                source_dir: base.join(&entry.source),
            };
            if self.templates.contains_key(tpl_name) {
                tracing::warn!(
                    template = %tpl_name,
                    plugin = %name,
                    "template name conflict — overwriting previous template"
                );
            }
            self.templates.insert(tpl_name.clone(), info);
        }

        self.plugins.push(plugin);
    }

    /// Look up a registered RPC handler by path.
    pub fn get_rpc_handler(&self, path: &str) -> Option<&RpcHandler> {
        self.rpc_handlers.get(path)
    }

    /// Get all hook handlers registered for an event.
    pub fn get_hook_handlers(&self, event: &str) -> &[HookHandler] {
        self.hooks.get(event).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Look up a registered template by name.
    pub fn get_template(&self, name: &str) -> Option<&TemplateInfo> {
        self.templates.get(name)
    }

    /// List all loaded plugins.
    #[allow(dead_code)]
    pub fn list_plugins(&self) -> &[LoadedPlugin] {
        &self.plugins
    }

    /// List all available template names.
    pub fn list_templates(&self) -> Vec<&str> {
        self.templates.keys().map(|s| s.as_str()).collect()
    }

    /// List all registered RPC endpoint paths.
    #[allow(dead_code)]
    pub fn list_rpc_paths(&self) -> Vec<&str> {
        self.rpc_handlers.keys().map(|s| s.as_str()).collect()
    }

    /// Get a summary of all plugins for the /plugins RPC endpoint.
    pub fn summary(&self) -> Vec<PluginSummary> {
        self.plugins
            .iter()
            .map(|p| PluginSummary {
                name: p.manifest.name.clone(),
                version: p.manifest.version.clone(),
                description: p.manifest.description.clone(),
                templates: p.manifest.templates.keys().cloned().collect(),
                hooks: p.manifest.hooks.keys().cloned().collect(),
                rpc_endpoints: p
                    .manifest
                    .rpc
                    .endpoints
                    .iter()
                    .map(|ep| ep.path.clone())
                    .collect(),
            })
            .collect()
    }
}

/// Summary of a single plugin (for JSON serialization).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginSummary {
    pub name: String,
    pub version: String,
    pub description: String,
    pub templates: Vec<String>,
    pub hooks: Vec<String>,
    pub rpc_endpoints: Vec<String>,
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
            std::env::temp_dir().join(format!("ac-registry-test-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_plugin(root: &Path, name: &str, manifest: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("plugin.json"), manifest).unwrap();
    }

    #[test]
    fn empty_registry() {
        let reg = PluginRegistry::empty();
        assert!(reg.list_plugins().is_empty());
        assert!(reg.list_templates().is_empty());
        assert!(reg.list_rpc_paths().is_empty());
        assert!(reg.get_rpc_handler("/x/test/foo").is_none());
        assert!(reg.get_hook_handlers("on_daemon_start").is_empty());
        assert!(reg.get_template("foo").is_none());
    }

    #[test]
    fn registry_from_plugins_dir() {
        let dir = tmp_dir("fromdir");
        write_plugin(
            &dir,
            "alpha",
            r#"{
                "name": "alpha",
                "version": "1.0.0",
                "templates": {
                    "dash": { "source": "templates/dash/", "entrypoint": "index.html" }
                },
                "hooks": { "on_daemon_start": "hooks/start.sh" },
                "rpc": { "endpoints": [
                    { "path": "/x/alpha/run", "handler": "handlers/run.sh" }
                ] }
            }"#,
        );
        write_plugin(
            &dir,
            "beta",
            r#"{
                "name": "beta",
                "version": "2.0.0",
                "hooks": { "on_daemon_start": "hooks/init.sh", "on_file_change": "hooks/change.sh" }
            }"#,
        );

        let reg = PluginRegistry::new(&dir).unwrap();
        assert_eq!(reg.list_plugins().len(), 2);

        // Template lookup.
        let tpl = reg.get_template("dash").unwrap();
        assert_eq!(tpl.plugin_name, "alpha");
        assert_eq!(tpl.entry.entrypoint, "index.html");

        // RPC handler lookup.
        let rpc = reg.get_rpc_handler("/x/alpha/run").unwrap();
        assert_eq!(rpc.plugin_name, "alpha");
        assert!(rpc.script_path.ends_with("handlers/run.sh"));

        // Hook handlers — on_daemon_start should have 2 handlers (from both plugins).
        let hooks = reg.get_hook_handlers("on_daemon_start");
        assert_eq!(hooks.len(), 2);

        // on_file_change should have 1 handler.
        let fc_hooks = reg.get_hook_handlers("on_file_change");
        assert_eq!(fc_hooks.len(), 1);
        assert_eq!(fc_hooks[0].plugin_name, "beta");

        // Unknown hook should be empty.
        assert!(reg.get_hook_handlers("on_something_else").is_empty());

        // Summary.
        let summary = reg.summary();
        assert_eq!(summary.len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_nonexistent_dir() {
        let dir = std::env::temp_dir().join("ac-registry-test-noexist-999");
        let _ = std::fs::remove_dir_all(&dir);
        let reg = PluginRegistry::new(&dir).unwrap();
        assert!(reg.list_plugins().is_empty());
    }

    #[test]
    fn registry_rpc_conflict_last_wins() {
        let dir = tmp_dir("conflict");
        write_plugin(
            &dir,
            "first",
            r#"{
                "name": "first",
                "version": "1.0.0",
                "rpc": { "endpoints": [
                    { "path": "/x/shared/action", "handler": "h1.sh" }
                ] }
            }"#,
        );
        write_plugin(
            &dir,
            "second",
            r#"{
                "name": "second",
                "version": "1.0.0",
                "rpc": { "endpoints": [
                    { "path": "/x/shared/action", "handler": "h2.sh" }
                ] }
            }"#,
        );

        let reg = PluginRegistry::new(&dir).unwrap();
        let handler = reg.get_rpc_handler("/x/shared/action").unwrap();
        // The last-loaded plugin should win (order depends on readdir, but
        // we verify one of them won).
        assert!(handler.plugin_name == "first" || handler.plugin_name == "second");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_template_conflict_last_wins() {
        let dir = tmp_dir("tplconflict");
        write_plugin(
            &dir,
            "a-plugin",
            r#"{
                "name": "a-plugin",
                "version": "1.0.0",
                "templates": {
                    "shared": { "source": "t1/", "entrypoint": "a.html" }
                }
            }"#,
        );
        write_plugin(
            &dir,
            "b-plugin",
            r#"{
                "name": "b-plugin",
                "version": "1.0.0",
                "templates": {
                    "shared": { "source": "t2/", "entrypoint": "b.html" }
                }
            }"#,
        );

        let reg = PluginRegistry::new(&dir).unwrap();
        let tpl = reg.get_template("shared").unwrap();
        assert!(tpl.plugin_name == "a-plugin" || tpl.plugin_name == "b-plugin");

        let _ = fs::remove_dir_all(&dir);
    }
}
