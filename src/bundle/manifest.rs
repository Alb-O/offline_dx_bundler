//! Loading and interpreting the build-time offline manifest.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::project::OfflineProjectLayout;

/// Deserialised representation of the build-time offline manifest.
#[derive(Debug, Deserialize)]
pub struct OfflineManifest {
    /// Optional site root specified in the manifest JSON.
    #[serde(default)]
    pub site_root: Option<String>,
    /// Hero assets required by the offline launcher UI.
    #[serde(default)]
    pub hero_assets: Vec<String>,
    /// Entries discovered during the build.
    #[serde(default)]
    pub entries: Vec<OfflineEntry>,
}

/// Offline entry contained within the manifest.
#[derive(Debug, Deserialize)]
pub struct OfflineEntry {
    /// Collection identifier the entry belongs to.
    #[serde(default)]
    pub collection_id: String,
    /// Entry identifier within the collection.
    #[serde(default)]
    pub entry_id: String,
    /// Asset paths referenced by the entry body.
    #[serde(default)]
    pub asset_paths: Vec<String>,
}

/// Load an offline manifest from disk.
pub fn load_manifest(path: &Path) -> Result<OfflineManifest> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("manifest not found at {}", path.display()))?;
    let manifest: OfflineManifest =
        serde_json::from_str(&content).context("failed to parse offline manifest JSON")?;
    Ok(manifest)
}

/// Determine the resolved site root and prefix from the manifest information.
pub fn resolve_site_root(
    layout: &OfflineProjectLayout,
    manifest: &OfflineManifest,
) -> (PathBuf, String) {
    let offline_root = PathBuf::from(layout.offline_bundle_root);
    let site_raw = manifest
        .site_root
        .as_deref()
        .unwrap_or(layout.offline_site_root);
    let segments: Vec<&str> = site_raw
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();

    if segments.is_empty() {
        (offline_root, String::new())
    } else {
        let mut root = offline_root.clone();
        for segment in &segments {
            root.push(segment);
        }
        let prefix = segments.join("/");
        (root, prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> OfflineProjectLayout<'static> {
        OfflineProjectLayout {
            entry_assets_dir: "assets",
            entry_markdown_file: "index.md",
            collection_metadata_file: "program.json",
            excluded_dir_name: "prod",
            excluded_path_fragment: "/prod/",
            collection_asset_literal_prefix: "/content/programs",
            offline_site_root: "site",
            collections_dir_name: "programs",
            offline_bundle_root: "target/offline-html",
            index_html_file: "index.html",
            target_dir: "target",
            offline_manifest_json: "offline_manifest.json",
        }
    }

    fn manifest_with_site_root(root: Option<&str>) -> OfflineManifest {
        OfflineManifest {
            site_root: root.map(|value| value.to_string()),
            hero_assets: Vec::new(),
            entries: Vec::new(),
        }
    }

    #[test]
    fn defaults_to_offline_site_root() {
        let manifest = manifest_with_site_root(None);
        let (root, prefix) = resolve_site_root(&layout(), &manifest);

        assert_eq!(root, PathBuf::from("target/offline-html").join("site"));
        assert_eq!(prefix, "site");
    }

    #[test]
    fn resolves_nested_site_root() {
        let manifest = manifest_with_site_root(Some("site/deep"));
        let (root, prefix) = resolve_site_root(&layout(), &manifest);

        assert_eq!(
            root,
            PathBuf::from("target/offline-html")
                .join("site")
                .join("deep")
        );
        assert_eq!(prefix, "site/deep");
    }
}
