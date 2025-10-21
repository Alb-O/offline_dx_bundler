//! Project configuration loader for describing offline bundle layout.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::project::OfflineProjectLayout;

const DEFAULT_CONFIG_FILE: &str = "offline.config.json";

/// Discoverable project configuration describing filesystem layout and output paths.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ProjectConfig {
    /// Relative path from the manifest directory to the authored collections.
    pub collections_dir: String,
    /// Optional JSON file describing which collections to include in builds.
    pub collections_local_path: String,
    /// Directory containing static assets for each collection.
    pub entry_assets_dir: String,
    /// Markdown filename that represents collection entries.
    pub entry_markdown_file: String,
    /// Metadata filename describing the collection (title, description, etc.).
    pub collection_metadata_file: String,
    /// Directory name containing assets that should be ignored for offline bundles.
    pub excluded_dir_name: String,
    /// Path fragment that signals a resource should be excluded from offline bundles.
    pub excluded_path_fragment: String,
    /// String literal prefix used when embedding assets in generated code.
    pub collection_asset_literal_prefix: String,
    /// Relative site root within the offline bundle output.
    pub offline_site_root: String,
    /// Directory name holding all collections.
    pub collections_dir_name: String,
    /// Path where the offline HTML bundle should be written.
    pub offline_bundle_root: String,
    /// File name of the application entry point HTML.
    pub index_html_file: String,
    /// Cargo target directory used during builds.
    pub target_dir: String,
    /// Name of the serialized offline manifest JSON file.
    pub offline_manifest_json: String,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            collections_dir: "../content/programs".into(),
            collections_local_path: "programs.local.json".into(),
            entry_assets_dir: "assets".into(),
            entry_markdown_file: "index.md".into(),
            collection_metadata_file: "collection.json".into(),
            excluded_dir_name: "dev".into(),
            excluded_path_fragment: "/dev/".into(),
            collection_asset_literal_prefix: "/content/programs".into(),
            offline_site_root: "site".into(),
            collections_dir_name: "programs".into(),
            offline_bundle_root: "target/offline-html".into(),
            index_html_file: "index.html".into(),
            target_dir: "target".into(),
            offline_manifest_json: "offline_manifest.json".into(),
        }
    }
}

impl ProjectConfig {
    /// Attempt to load configuration from the provided directory.
    ///
    /// When the configuration file does not exist or fails to parse we fallback to default
    /// values so downstream callers can continue operating with sensible assumptions.
    pub fn discover(manifest_dir: &Path) -> Self {
        let candidate = manifest_dir.join(DEFAULT_CONFIG_FILE);
        Self::from_path(&candidate).unwrap_or_default()
    }

    /// Read configuration from a specific JSON file.
    pub fn from_path(path: &Path) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Convert the configuration into an owned layout description.
    pub fn into_layout(self) -> OfflineProjectLayout {
        OfflineProjectLayout {
            entry_assets_dir: self.entry_assets_dir,
            entry_markdown_file: self.entry_markdown_file,
            collection_metadata_file: self.collection_metadata_file,
            excluded_dir_name: self.excluded_dir_name,
            excluded_path_fragment: self.excluded_path_fragment,
            collection_asset_literal_prefix: self.collection_asset_literal_prefix,
            offline_site_root: self.offline_site_root,
            collections_dir_name: self.collections_dir_name,
            offline_bundle_root: self.offline_bundle_root,
            index_html_file: self.index_html_file,
            target_dir: self.target_dir,
            offline_manifest_json: self.offline_manifest_json,
        }
    }

    /// Borrowing conversion into a layout, cloning the underlying strings.
    pub fn to_layout(&self) -> OfflineProjectLayout {
        OfflineProjectLayout {
            entry_assets_dir: self.entry_assets_dir.clone(),
            entry_markdown_file: self.entry_markdown_file.clone(),
            collection_metadata_file: self.collection_metadata_file.clone(),
            excluded_dir_name: self.excluded_dir_name.clone(),
            excluded_path_fragment: self.excluded_path_fragment.clone(),
            collection_asset_literal_prefix: self.collection_asset_literal_prefix.clone(),
            offline_site_root: self.offline_site_root.clone(),
            collections_dir_name: self.collections_dir_name.clone(),
            offline_bundle_root: self.offline_bundle_root.clone(),
            index_html_file: self.index_html_file.clone(),
            target_dir: self.target_dir.clone(),
            offline_manifest_json: self.offline_manifest_json.clone(),
        }
    }
}

impl ProjectConfig {
    /// Path relative to the manifest root for authored collections.
    pub fn collections_dir_path(&self, manifest_dir: &Path) -> PathBuf {
        manifest_dir.join(&self.collections_dir)
    }

    /// Path to the local selection file.
    pub fn collections_local_file(&self, manifest_dir: &Path) -> PathBuf {
        manifest_dir
            .join(&self.collections_dir)
            .join(&self.collections_local_path)
    }
}
