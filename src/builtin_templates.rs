//! Built-in templates that ship with the browsercli binary.
//!
//! These templates are embedded at compile time so they work without installing
//! any plugins.  Each template is a single `index.html` that uses CDN-hosted
//! libraries (zero build step).

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;

/// Metadata for a built-in template.
#[allow(dead_code)]
struct BuiltinTemplate {
    description: &'static str,
    html: &'static str,
}

fn builtin_templates() -> HashMap<&'static str, BuiltinTemplate> {
    let mut m = HashMap::new();
    m.insert(
        "tailwind",
        BuiltinTemplate {
            description: "Tailwind CSS v4 starter with responsive layout",
            html: include_str!("builtin_templates/tailwind.html"),
        },
    );
    m.insert(
        "dashboard",
        BuiltinTemplate {
            description: "Dashboard layout with Tailwind CSS + DaisyUI components",
            html: include_str!("builtin_templates/dashboard.html"),
        },
    );
    m.insert(
        "chart",
        BuiltinTemplate {
            description: "Chart.js data visualization starter",
            html: include_str!("builtin_templates/chart.html"),
        },
    );
    m.insert(
        "form",
        BuiltinTemplate {
            description: "Interactive form with Tailwind CSS + Alpine.js",
            html: include_str!("builtin_templates/form.html"),
        },
    );
    m
}

/// List all available built-in template names.
pub fn list_builtin_templates() -> Vec<&'static str> {
    let mut names: Vec<&str> = builtin_templates().keys().copied().collect();
    names.sort();
    names
}

/// Apply a built-in template by writing its `index.html` to the destination directory.
///
/// Returns `Ok(true)` if the template was found and applied, `Ok(false)` if
/// the name is not a built-in template (caller should fall through to plugin lookup).
pub async fn apply_builtin_template(name: &str, dest: &Path) -> Result<bool> {
    let templates = builtin_templates();
    let tpl = match templates.get(name) {
        Some(t) => t,
        None => return Ok(false),
    };

    tokio::fs::create_dir_all(dest).await?;
    tokio::fs::write(dest.join("index.html"), tpl.html).await?;

    tracing::info!(template = %name, dest = %dest.display(), "applied built-in template");
    Ok(true)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_templates_present() {
        let names = list_builtin_templates();
        assert!(names.contains(&"tailwind"));
        assert!(names.contains(&"dashboard"));
        assert!(names.contains(&"chart"));
        assert!(names.contains(&"form"));
    }

    #[test]
    fn templates_contain_valid_html() {
        let templates = builtin_templates();
        for (name, tpl) in &templates {
            assert!(
                tpl.html.contains("<!DOCTYPE html>"),
                "template '{}' missing DOCTYPE",
                name
            );
            assert!(
                tpl.html.contains("</html>"),
                "template '{}' missing closing html tag",
                name
            );
        }
    }

    #[test]
    fn descriptions_non_empty() {
        let templates = builtin_templates();
        for (name, tpl) in &templates {
            assert!(
                !tpl.description.is_empty(),
                "template '{}' has empty description",
                name
            );
        }
    }

    #[tokio::test]
    async fn apply_known_template() {
        let dir = std::env::temp_dir().join(format!(
            "browsercli-builtin-test-{}-apply",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);

        let applied = apply_builtin_template("tailwind", &dir).await.unwrap();
        assert!(applied);
        assert!(dir.join("index.html").exists());

        let content = std::fs::read_to_string(dir.join("index.html")).unwrap();
        assert!(content.contains("tailwindcss"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn apply_unknown_returns_false() {
        let dir = std::env::temp_dir().join(format!(
            "browsercli-builtin-test-{}-unknown",
            std::process::id()
        ));
        let applied = apply_builtin_template("nonexistent", &dir).await.unwrap();
        assert!(!applied);
    }
}
