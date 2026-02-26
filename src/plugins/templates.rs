//! Template copy logic.
//!
//! Copies a plugin template's source directory into the serve directory,
//! skipping hidden files and common build artifacts.

use anyhow::{Context, Result};
use std::path::Path;

use crate::plugins::registry::TemplateInfo;

/// Directories and files to skip when copying templates.
const SKIP_NAMES: &[&str] = &[
    "node_modules",
    ".git",
    ".svn",
    "target",
    "__pycache__",
    ".DS_Store",
    "Thumbs.db",
];

/// Apply a template by copying its source directory into the destination.
///
/// Recursively copies files, skipping hidden files/dirs (starting with `.`)
/// and common build artifacts. Validates that the entrypoint exists after copy.
pub async fn apply_template(template: &TemplateInfo, dest: &Path) -> Result<()> {
    let source = &template.source_dir;

    if !source.is_dir() {
        anyhow::bail!(
            "template source '{}' does not exist or is not a directory",
            source.display()
        );
    }

    // Ensure destination exists.
    tokio::fs::create_dir_all(dest)
        .await
        .with_context(|| format!("create destination dir: {}", dest.display()))?;

    // Recursive copy.
    copy_dir_recursive(source, dest).await.with_context(|| {
        format!(
            "copy template '{}' from {} to {}",
            template.template_name,
            source.display(),
            dest.display()
        )
    })?;

    // Validate entrypoint.
    let entrypoint = dest.join(&template.entry.entrypoint);
    if !entrypoint.exists() {
        anyhow::bail!(
            "template entrypoint '{}' not found after copy (expected at {})",
            template.entry.entrypoint,
            entrypoint.display()
        );
    }

    tracing::info!(
        template = %template.template_name,
        plugin = %template.plugin_name,
        dest = %dest.display(),
        "applied template"
    );

    Ok(())
}

/// Recursively copy a directory, skipping hidden and ignored entries.
async fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    let mut entries = tokio::fs::read_dir(src).await?;

    while let Some(entry) = entries.next_entry().await? {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        // Skip hidden files/dirs.
        if name.starts_with('.') {
            continue;
        }

        // Skip common build artifacts.
        if SKIP_NAMES.iter().any(|&s| s == name.as_ref()) {
            continue;
        }

        let src_path = entry.path();
        let dst_path = dst.join(&file_name);

        let file_type = entry.file_type().await?;

        if file_type.is_dir() {
            tokio::fs::create_dir_all(&dst_path).await?;
            Box::pin(copy_dir_recursive(&src_path, &dst_path)).await?;
        } else if file_type.is_file() {
            tokio::fs::copy(&src_path, &dst_path).await?;
        }
        // Skip symlinks — don't follow them for security.
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::registry::TemplateInfo;
    use crate::plugins::TemplateEntry;
    use std::fs;
    use std::path::PathBuf;

    fn tmp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ac-template-test-{}-{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_template_info(source_dir: &Path, entrypoint: &str) -> TemplateInfo {
        TemplateInfo {
            plugin_name: "test-plugin".to_string(),
            template_name: "test".to_string(),
            entry: TemplateEntry {
                description: "test template".to_string(),
                source: "src/".to_string(),
                entrypoint: entrypoint.to_string(),
            },
            source_dir: source_dir.to_path_buf(),
        }
    }

    #[tokio::test]
    async fn copy_basic_template() {
        let dir = tmp_dir("basic");
        let src = dir.join("source");
        let dst = dir.join("dest");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("index.html"), "<h1>Hello</h1>").unwrap();
        fs::write(src.join("style.css"), "body {}").unwrap();

        let info = make_template_info(&src, "index.html");
        apply_template(&info, &dst).await.unwrap();

        assert!(dst.join("index.html").exists());
        assert!(dst.join("style.css").exists());
        assert_eq!(
            fs::read_to_string(dst.join("index.html")).unwrap(),
            "<h1>Hello</h1>"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn copy_nested_directories() {
        let dir = tmp_dir("nested");
        let src = dir.join("source");
        let dst = dir.join("dest");
        fs::create_dir_all(src.join("js")).unwrap();
        fs::create_dir_all(src.join("css")).unwrap();
        fs::write(src.join("index.html"), "<!DOCTYPE html>").unwrap();
        fs::write(src.join("js/app.js"), "console.log('hi')").unwrap();
        fs::write(src.join("css/main.css"), "* { margin: 0 }").unwrap();

        let info = make_template_info(&src, "index.html");
        apply_template(&info, &dst).await.unwrap();

        assert!(dst.join("index.html").exists());
        assert!(dst.join("js/app.js").exists());
        assert!(dst.join("css/main.css").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn skips_hidden_files() {
        let dir = tmp_dir("hidden");
        let src = dir.join("source");
        let dst = dir.join("dest");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("index.html"), "ok").unwrap();
        fs::write(src.join(".hidden"), "secret").unwrap();
        fs::create_dir_all(src.join(".hidden-dir")).unwrap();
        fs::write(src.join(".hidden-dir/file.txt"), "secret").unwrap();

        let info = make_template_info(&src, "index.html");
        apply_template(&info, &dst).await.unwrap();

        assert!(dst.join("index.html").exists());
        assert!(!dst.join(".hidden").exists());
        assert!(!dst.join(".hidden-dir").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn skips_node_modules_and_git() {
        let dir = tmp_dir("skipnm");
        let src = dir.join("source");
        let dst = dir.join("dest");
        fs::create_dir_all(src.join("node_modules/dep")).unwrap();
        fs::write(src.join("node_modules/dep/index.js"), "x").unwrap();
        fs::create_dir_all(src.join(".git/objects")).unwrap();
        fs::create_dir_all(src.join("target")).unwrap();
        fs::write(src.join("index.html"), "ok").unwrap();

        let info = make_template_info(&src, "index.html");
        apply_template(&info, &dst).await.unwrap();

        assert!(dst.join("index.html").exists());
        assert!(!dst.join("node_modules").exists());
        assert!(!dst.join(".git").exists());
        assert!(!dst.join("target").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn rejects_missing_source_dir() {
        let dir = tmp_dir("nosource");
        let dst = dir.join("dest");
        let src = dir.join("nonexistent");

        let info = make_template_info(&src, "index.html");
        let err = apply_template(&info, &dst).await.unwrap_err();
        assert!(err.to_string().contains("does not exist"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn rejects_missing_entrypoint() {
        let dir = tmp_dir("noentry");
        let src = dir.join("source");
        let dst = dir.join("dest");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("other.html"), "not the entrypoint").unwrap();

        let info = make_template_info(&src, "index.html");
        let err = apply_template(&info, &dst).await.unwrap_err();
        assert!(err.to_string().contains("entrypoint"));

        let _ = fs::remove_dir_all(&dir);
    }
}
