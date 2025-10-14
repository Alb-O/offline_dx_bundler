//! Data structures produced while preparing an offline bundle.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Metadata describing a training program parsed from `program.json`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramMetaRecord {
  /// Program title taken from the frontmatter metadata file.
  pub title: String,
  /// Optional program description rendered alongside the title.
  pub description: Option<String>,
  /// Optional semantic version string attached to the program.
  pub version: Option<String>,
  /// Optional asset slug used to construct asset paths.
  pub asset_slug: Option<String>,
  /// Optional hero asset path to display in listings.
  pub hero_image: Option<String>,
}

/// Optional frontmatter fields attached to module markdown files.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct ModuleFrontmatterRecord {
  /// Module title rendered in the offline experience.
  pub title: Option<String>,
  /// Optional section grouping for the module.
  pub section: Option<String>,
  /// Explicit ordering override supplied in authored content.
  pub order: Option<usize>,
}

/// Structured representation of a program and its discovered modules.
#[derive(Debug, Clone, Serialize)]
pub struct ProgramCatalogRecord {
  /// Stable identifier for the program.
  pub id: String,
  /// Metadata describing the program.
  pub meta: ProgramMetaRecord,
  /// Modules discovered for the program.
  pub modules: Vec<ModuleRecord>,
}

/// Rendered module metadata for catalog presentation.
#[derive(Debug, Clone, Serialize)]
pub struct ModuleRecord {
  /// Stable identifier for the module.
  pub id: String,
  /// Human readable module title.
  pub title: String,
  /// Optional section grouping the module belongs to.
  pub section: Option<String>,
  /// Sequence value used to sort modules.
  pub sequence: usize,
  /// Path to the markdown source file that produced the module body.
  pub source: String,
}

/// Representation of a program asset required by the offline bundle.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AssetEntry {
  /// Constant name generated for the asset entry.
  pub const_name: String,
  /// Asset path as embedded in the generated Rust code.
  pub literal_path: String,
  /// Program identifier associated with the asset.
  pub program_id: String,
  /// Relative path of the asset within the program directory.
  pub relative_path: String,
}

impl AssetEntry {
  /// Relative path within the asset mirror for this entry.
  pub fn mirror_relative_path(&self) -> PathBuf {
    PathBuf::from(&self.program_id).join(&self.relative_path)
  }

  /// Source path of the asset relative to the authored programs directory.
  pub fn source_path(&self, programs_dir: &Path) -> PathBuf {
    programs_dir
      .join(&self.program_id)
      .join(&self.relative_path)
  }
}

/// Fully rendered offline module representation.
#[derive(Debug, Clone)]
pub struct OfflineModuleRecord {
  /// Program identifier the module belongs to.
  pub program_id: String,
  /// Module identifier.
  pub module_id: String,
  /// Rendered HTML body for the module.
  pub body: String,
  /// Relative asset paths referenced by the module.
  pub asset_paths: Vec<String>,
}

/// Serializable summary of an offline module.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OfflineModuleSummary {
  /// Program identifier the module belongs to.
  pub program_id: String,
  /// Module identifier.
  pub module_id: String,
  /// Relative asset paths referenced by the module.
  pub asset_paths: Vec<String>,
}

/// Serializable summary of the offline manifest written to disk.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OfflineManifestSummary {
  /// Relative path to the offline site root inside the bundle output.
  pub site_root: String,
  /// Summary of modules included in the manifest.
  pub modules: Vec<OfflineModuleSummary>,
  /// Collected hero asset paths required by the offline experience.
  pub hero_assets: Vec<String>,
}

/// Complete manifest generation output returned by [`crate::OfflineBuilder`].
#[derive(Debug)]
pub struct ManifestGenerationResult {
  /// Records describing the discovered programs and modules.
  pub program_catalog: Vec<ProgramCatalogRecord>,
  /// Complete representation of modules required for the offline bundle.
  pub offline_modules: Vec<OfflineModuleRecord>,
  /// Mapping of program and relative path to offline asset entries.
  pub asset_map: BTreeMap<(String, String), AssetEntry>,
  /// Hero assets collected while scanning program metadata.
  pub hero_asset_paths: BTreeSet<String>,
  /// Match arms used to generate hero asset lookup code.
  pub hero_match_arms: Vec<String>,
}
