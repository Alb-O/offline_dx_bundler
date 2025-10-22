//! Data structures produced while preparing an offline bundle.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Metadata describing an authored collection parsed from the metadata file.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionMetaRecord {
  /// Collection title taken from the frontmatter metadata file.
  pub title: String,
  /// Optional collection description rendered alongside the title.
  pub description: Option<String>,
  /// Optional semantic version string attached to the collection.
  pub version: Option<String>,
  /// Optional asset slug used to construct asset paths.
  pub asset_slug: Option<String>,
  /// Optional hero asset path to display in listings.
  pub hero_image: Option<String>,
}

/// Optional frontmatter fields attached to entry markdown files.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct EntryFrontmatterRecord {
  /// Entry title rendered in the offline experience.
  pub title: Option<String>,
  /// Optional section grouping for the entry.
  pub section: Option<String>,
  /// Explicit ordering override supplied in authored content.
  pub order: Option<usize>,
}

/// Structured representation of a collection and its discovered entries.
#[derive(Debug, Clone, Serialize)]
pub struct CollectionCatalogRecord {
  /// Stable identifier for the collection.
  pub id: String,
  /// Metadata describing the collection.
  pub meta: CollectionMetaRecord,
  /// Entries discovered for the collection.
  pub entries: Vec<EntryRecord>,
}

/// Rendered entry metadata for catalog presentation.
#[derive(Debug, Clone, Serialize)]
pub struct EntryRecord {
  /// Stable identifier for the entry.
  pub id: String,
  /// Human readable entry title.
  pub title: String,
  /// Optional section grouping the entry belongs to.
  pub section: Option<String>,
  /// Sequence value used to sort modules.
  pub sequence: usize,
  /// Path to the markdown source file that produced the entry body.
  pub source: String,
}

/// Representation of a collection asset required by the offline bundle.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AssetEntry {
  /// Constant name generated for the asset entry.
  pub const_name: String,
  /// Asset path as embedded in the generated Rust code.
  pub literal_path: String,
  /// Collection identifier associated with the asset.
  pub collection_id: String,
  /// Relative path of the asset within the collection directory.
  pub relative_path: String,
}

impl AssetEntry {
  /// Relative path within the asset mirror for this entry.
  pub fn mirror_relative_path(&self) -> PathBuf {
    PathBuf::from(&self.collection_id).join(&self.relative_path)
  }

  /// Source path of the asset relative to the authored collections directory.
  pub fn source_path(&self, collections_dir: &Path) -> PathBuf {
    collections_dir
      .join(&self.collection_id)
      .join(&self.relative_path)
  }
}

/// Fully rendered offline entry representation.
#[derive(Debug, Clone)]
pub struct OfflineEntryRecord {
  /// Collection identifier the entry belongs to.
  pub collection_id: String,
  /// Entry identifier.
  pub entry_id: String,
  /// Rendered HTML body for the entry.
  pub body: String,
  /// Relative asset paths referenced by the entry.
  pub asset_paths: Vec<String>,
}

/// Serializable summary of an offline entry.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OfflineEntrySummary {
  /// Collection identifier the entry belongs to.
  pub collection_id: String,
  /// Entry identifier.
  pub entry_id: String,
  /// Relative asset paths referenced by the entry.
  pub asset_paths: Vec<String>,
}

/// Serializable summary of the offline manifest written to disk.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OfflineManifestSummary {
  /// Relative path to the offline site root inside the bundle output.
  pub site_root: String,
  /// Summary of entries included in the manifest.
  pub entries: Vec<OfflineEntrySummary>,
  /// Collected hero asset paths required by the offline experience.
  pub hero_assets: Vec<String>,
}

/// Context for asset collection operations.
#[derive(Debug)]
pub struct AssetCollectionContext<'a> {
  /// Mapping of collection and relative path to offline asset entries.
  pub asset_map: &'a mut BTreeMap<(String, String), AssetEntry>,
  /// Set of used constant names for deduplication.
  pub used_names: &'a mut BTreeSet<String>,
  /// Hero assets collected while scanning collection metadata.
  pub hero_asset_paths: &'a mut BTreeSet<String>,
  /// Match arms used to generate hero asset lookup code.
  pub hero_match_arms: &'a mut Vec<String>,
}

/// Context for manifest generation operations.
#[derive(Debug)]
pub struct ManifestGenerationContext<'a> {
  /// Asset collection context.
  pub assets: AssetCollectionContext<'a>,
  /// Records describing the discovered collections and entries.
  pub collection_catalog: &'a mut Vec<CollectionCatalogRecord>,
  /// Complete representation of entries required for the offline bundle.
  pub offline_entries: &'a mut Vec<OfflineEntryRecord>,
}

/// Configuration for asset scanning operations.
#[derive(Debug, Clone)]
pub struct AssetScanningConfig<'a> {
  /// Name of directories to exclude from scanning.
  pub excluded_dir_name: &'a str,
  /// Name of entry assets directory.
  pub entry_assets_dir: &'a str,
  /// Name of entry markdown file.
  pub entry_markdown_file: &'a str,
  /// Path fragment to exclude from asset paths.
  pub excluded_path_fragment: &'a str,
  /// Prefix for collection asset literal paths.
  pub collection_asset_literal_prefix: &'a str,
  /// Name of collection metadata file.
  pub collection_metadata_file: &'a str,
}

/// Complete manifest generation output returned by [`crate::OfflineBuilder`].
#[derive(Debug)]
pub struct ManifestGenerationResult {
  /// Records describing the discovered collections and entries.
  pub collection_catalog: Vec<CollectionCatalogRecord>,
  /// Complete representation of entries required for the offline bundle.
  pub offline_entries: Vec<OfflineEntryRecord>,
  /// Mapping of collection and relative path to offline asset entries.
  pub asset_map: BTreeMap<(String, String), AssetEntry>,
  /// Hero assets collected while scanning collection metadata.
  pub hero_asset_paths: BTreeSet<String>,
  /// Match arms used to generate hero asset lookup code.
  pub hero_match_arms: Vec<String>,
}
