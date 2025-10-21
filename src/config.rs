//! Project configuration loader for describing offline bundle layout.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

use crate::project::OfflineProjectLayout;

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

/// Optional configuration overrides embedded within collection metadata files.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionConfigOverrides {
    /// Relative path from the manifest directory to the authored collections.
    #[serde(default)]
    pub collections_dir: Option<String>,
    /// File containing the optional collection inclusion list.
    #[serde(default)]
    pub collections_local_path: Option<String>,
    /// Directory containing static assets for each collection entry.
    #[serde(default)]
    pub entry_assets_dir: Option<String>,
    /// Markdown filename that represents collection entries.
    #[serde(default)]
    pub entry_markdown_file: Option<String>,
    /// Metadata filename describing a collection.
    #[serde(default)]
    pub collection_metadata_file: Option<String>,
    /// Directory that should be excluded from offline bundles.
    #[serde(default)]
    pub excluded_dir_name: Option<String>,
    /// Path fragment that marks resources to skip from offline bundles.
    #[serde(default)]
    pub excluded_path_fragment: Option<String>,
    /// Literal prefix used when embedding assets in generated code.
    #[serde(default)]
    pub collection_asset_literal_prefix: Option<String>,
    /// Relative site root within the offline bundle output.
    #[serde(default)]
    pub offline_site_root: Option<String>,
    /// Directory name that stores all collections inside the offline bundle.
    #[serde(default)]
    pub collections_dir_name: Option<String>,
    /// Output directory for the offline HTML bundle.
    #[serde(default)]
    pub offline_bundle_root: Option<String>,
    /// File name of the application entry point HTML.
    #[serde(default)]
    pub index_html_file: Option<String>,
    /// Cargo target directory used during builds.
    #[serde(default)]
    pub target_dir: Option<String>,
    /// Name of the serialized offline manifest JSON file.
    #[serde(default)]
    pub offline_manifest_json: Option<String>,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            collections_dir: "../content/programs".into(),
            collections_local_path: "collections.local.json".into(),
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
    /// When configuration overrides do not exist or fail to parse we fall back to default
    /// values so downstream callers can continue operating with sensible assumptions.
    pub fn discover(manifest_dir: &Path) -> Self {
        let mut config = Self::default();

        let root_metadata_path = manifest_dir
            .join(&config.collections_dir)
            .join(&config.collection_metadata_file);

        if let Some(overrides) = load_config_overrides(&root_metadata_path) {
            config.apply_overrides(&overrides);
        }

        config
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

    fn apply_overrides(&mut self, overrides: &CollectionConfigOverrides) {
        if let Some(value) = &overrides.collections_dir {
            self.collections_dir = value.clone();
        }
        if let Some(value) = &overrides.collections_local_path {
            self.collections_local_path = value.clone();
        }
        if let Some(value) = &overrides.entry_assets_dir {
            self.entry_assets_dir = value.clone();
        }
        if let Some(value) = &overrides.entry_markdown_file {
            self.entry_markdown_file = value.clone();
        }
        if let Some(value) = &overrides.collection_metadata_file {
            self.collection_metadata_file = value.clone();
        }
        if let Some(value) = &overrides.excluded_dir_name {
            self.excluded_dir_name = value.clone();
        }
        if let Some(value) = &overrides.excluded_path_fragment {
            self.excluded_path_fragment = value.clone();
        }
        if let Some(value) = &overrides.collection_asset_literal_prefix {
            self.collection_asset_literal_prefix = value.clone();
        }
        if let Some(value) = &overrides.offline_site_root {
            self.offline_site_root = value.clone();
        }
        if let Some(value) = &overrides.collections_dir_name {
            self.collections_dir_name = value.clone();
        }
        if let Some(value) = &overrides.offline_bundle_root {
            self.offline_bundle_root = value.clone();
        }
        if let Some(value) = &overrides.index_html_file {
            self.index_html_file = value.clone();
        }
        if let Some(value) = &overrides.target_dir {
            self.target_dir = value.clone();
        }
        if let Some(value) = &overrides.offline_manifest_json {
            self.offline_manifest_json = value.clone();
        }
    }
}

impl CollectionConfigOverrides {
    /// Apply overrides that are valid for individual collection layouts.
    pub fn apply_to_layout(&self, layout: &mut OfflineProjectLayout) {
        if let Some(value) = &self.entry_assets_dir {
            layout.entry_assets_dir = value.clone();
        }
        if let Some(value) = &self.entry_markdown_file {
            layout.entry_markdown_file = value.clone();
        }
        if let Some(value) = &self.collection_metadata_file {
            layout.collection_metadata_file = value.clone();
        }
        if let Some(value) = &self.excluded_dir_name {
            layout.excluded_dir_name = value.clone();
        }
        if let Some(value) = &self.excluded_path_fragment {
            layout.excluded_path_fragment = value.clone();
        }
        if let Some(value) = &self.collection_asset_literal_prefix {
            layout.collection_asset_literal_prefix = value.clone();
        }
    }

    /// Returns true when no overrides are specified.
    pub fn is_empty(&self) -> bool {
        self.collections_dir.is_none()
            && self.collections_local_path.is_none()
            && self.entry_assets_dir.is_none()
            && self.entry_markdown_file.is_none()
            && self.collection_metadata_file.is_none()
            && self.excluded_dir_name.is_none()
            && self.excluded_path_fragment.is_none()
            && self.collection_asset_literal_prefix.is_none()
            && self.offline_site_root.is_none()
            && self.collections_dir_name.is_none()
            && self.offline_bundle_root.is_none()
            && self.index_html_file.is_none()
            && self.target_dir.is_none()
            && self.offline_manifest_json.is_none()
    }
}

/// Attempt to read configuration overrides from a metadata document.
pub fn load_config_overrides(path: &Path) -> Option<CollectionConfigOverrides> {
    load_document(path)
        .map(|(_, overrides)| overrides)
        .filter(|overrides| !overrides.is_empty())
}

/// Load metadata and any configuration overrides from a document.
pub fn load_metadata_with_overrides<T>(
    path: &Path,
) -> Option<(T, CollectionConfigOverrides)>
where
    T: DeserializeOwned,
{
    let (payload, overrides) = load_document(path)?;
    let meta = serde_json::from_value(payload).ok()?;
    Some((meta, overrides))
}

/// Read a collection document returning the payload and any embedded overrides.
pub fn load_document(path: &Path) -> Option<(Value, CollectionConfigOverrides)> {
    let content = fs::read_to_string(path).ok()?;
    split_document(&content)
}

fn split_document(content: &str) -> Option<(Value, CollectionConfigOverrides)> {
    let mut value: Value = serde_json::from_str(content).ok()?;
    let overrides = if let Some(object) = value.as_object_mut() {
        match object.remove("config") {
            Some(config_value) => serde_json::from_value(config_value).unwrap_or_default(),
            None => CollectionConfigOverrides::default(),
        }
    } else {
        CollectionConfigOverrides::default()
    };

    Some((value, overrides))
}
