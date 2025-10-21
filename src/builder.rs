//! Offline build orchestrator responsible for generating manifests and bundling assets.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use same_file::is_same_file;

use crate::asset_paths::make_offline_asset_path;
use crate::manifest::generate_offline_manifest;
use crate::models::{
    AssetEntry, ManifestGenerationResult, OfflineEntryRecord, OfflineEntrySummary,
    OfflineManifestSummary,
};
use crate::project::{OfflineBuildContext, OfflineProjectLayout};
use crate::selection::CollectionInclusion;

/// Generic build result type used across the crate.
pub type BuildResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Collection of generated artifacts required by the offline bundle.
pub struct OfflineArtifacts {
    /// Rust source defining the collection asset lookup table.
    pub asset_table_code: String,
    /// Rust source providing offline entry bodies and asset mappings.
    pub offline_manifest_code: String,
    /// Offline manifest serialised as prettified JSON.
    pub offline_manifest_json: String,
    /// Collection catalog JSON used by the launcher UI.
    pub collection_catalog_json: String,
    /// File system paths that should trigger rerunning the build script when changed.
    pub rerun_paths: Vec<PathBuf>,
}

/// High-level helper for generating offline manifests and preparing assets.
pub struct OfflineBuilder<'a> {
    context: OfflineBuildContext<'a>,
}

impl<'a> OfflineBuilder<'a> {
    /// Create a builder for the provided build context.
    pub fn new(context: OfflineBuildContext<'a>) -> Self {
        Self { context }
    }

    /// Generate the offline manifest, mirror referenced assets and return the resulting artifacts.
    pub fn build<S: CollectionInclusion>(&self, selection: &S) -> BuildResult<OfflineArtifacts> {
        let ManifestGenerationResult {
            collection_catalog,
            offline_entries,
            asset_map,
            hero_asset_paths,
            hero_match_arms,
        } = self.generate_manifest(selection)?;

        self.prepare_collection_asset_sources(&asset_map)?;

        let layout = &self.context.layout;
        let mirror_base = &self.context.asset_mirror_dir;
        let mirror_relative = match mirror_base.strip_prefix(self.context.manifest_dir) {
            Ok(path) => path,
            Err(_) => mirror_base.as_path(),
        };
        let mirror_prefix = format!(
            "/{}",
            mirror_relative
                .to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches('/')
        );

        let (asset_definitions, asset_match_entries) =
            render_collection_assets(&asset_map, &mirror_prefix);
        let hero_section = render_hero_match_section(&hero_match_arms);

        let asset_table_code = format!(
            r#"// Generated at build time by build tooling
use dioxus::prelude::Asset;

// Static asset definitions for all collections
{}

// Generated lookup function
fn get_collection_hero_asset(collection_id: &str) -> Option<&'static Asset> {{
    match collection_id {{
{}
    }}
}}

// Lookup for arbitrary collection assets referenced in markdown
#[allow(unreachable_patterns)]
pub(crate) fn get_collection_asset(collection_id: &str, relative_path: &str) -> Option<&'static Asset> {{
    match (collection_id, relative_path) {{
{}
        _ => None,
    }}
}}
"#,
            asset_definitions.join("\n"),
            hero_section,
            asset_match_entries.join("\n"),
        );

        let (offline_entry_code, offline_asset_code) =
            render_offline_entry_tables(layout, &offline_entries, &asset_map);

        let offline_manifest_code = format!(
            r#"// Generated at build time for the offline-html feature
use serde::{{Deserialize, Serialize}};

#[derive(Clone)]
pub struct OfflineEntry {{
    pub body: &'static str,
    pub assets: &'static [&'static str],
}}
{}

#[allow(dead_code)]
pub fn offline_entry(collection_id: &str, entry_id: &str) -> Option<OfflineEntry> {{
    match (collection_id, entry_id) {{
{}
    }}
}}

pub(crate) fn offline_entry_body(collection_id: &str, entry_id: &str) -> Option<&'static str> {{
    offline_entry(collection_id, entry_id).map(|record| record.body)
}}

pub(crate) fn offline_entry_assets(collection_id: &str, entry_id: &str) -> Option<&'static [&'static str]> {{
    offline_entry(collection_id, entry_id).map(|record| record.assets)
}}

#[allow(unreachable_patterns)]
pub(crate) fn offline_collection_asset(collection_id: &str, relative_path: &str) -> Option<&'static str> {{
    match (collection_id, relative_path) {{
{}
        _ => None,
    }}
}}
"#,
            offline_entry_code, offline_asset_code.0, offline_asset_code.1,
        );

        let offline_manifest_json = serde_json::to_string_pretty(&OfflineManifestSummary {
            site_root: layout.offline_site_root.clone(),
            entries: offline_entries
                .iter()
                .map(|entry| OfflineEntrySummary {
                    collection_id: entry.collection_id.clone(),
                    entry_id: entry.entry_id.clone(),
                    asset_paths: entry.asset_paths.clone(),
                })
                .collect(),
            hero_assets: hero_asset_paths.iter().cloned().collect(),
        })?;

        let collection_catalog_json = serde_json::to_string_pretty(&collection_catalog)?;

        let mut rerun_paths = vec![self.context.collections_dir.to_path_buf()];
        rerun_paths.push(self.context.collections_local_path.to_path_buf());
        append_collection_metadata_paths(self.context.collections_dir, &layout, &mut rerun_paths);

        Ok(OfflineArtifacts {
            asset_table_code,
            offline_manifest_code,
            offline_manifest_json,
            collection_catalog_json,
            rerun_paths,
        })
    }

    fn generate_manifest<S: CollectionInclusion>(
        &self,
        selection: &S,
    ) -> BuildResult<ManifestGenerationResult> {
        generate_offline_manifest(&self.context.layout, self.context.collections_dir, selection)
    }

    fn prepare_collection_asset_sources(
        &self,
        asset_map: &BTreeMap<(String, String), AssetEntry>,
    ) -> BuildResult<()> {
        let mirror_root = &self.context.asset_mirror_dir;
        let mut desired_relatives = BTreeSet::new();
        let mut available_assets = Vec::new();

        for entry in asset_map.values() {
            let source_path = entry.source_path(self.context.collections_dir);
            if !source_path.exists() {
                continue;
            }
            let relative_path = entry.mirror_relative_path();
            desired_relatives.insert(relative_path.clone());
            available_assets.push((source_path, relative_path));
        }

        if !mirror_root.exists() {
            fs::create_dir_all(mirror_root)?;
        }

        prune_mirror_tree(mirror_root, &desired_relatives)?;

        for (source, relative) in available_assets {
            let destination = mirror_root.join(&relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }

            install_collection_asset(&source, &destination)?;
        }

        Ok(())
    }
}

fn append_collection_metadata_paths(
    collections_dir: &Path,
    layout: &OfflineProjectLayout,
    rerun_paths: &mut Vec<PathBuf>,
) {
    if let Ok(entries) = fs::read_dir(collections_dir) {
        for entry in entries.flatten() {
            if entry.file_type().is_ok_and(|ft| ft.is_dir()) {
                let metadata = entry.path().join(&layout.collection_metadata_file);
                if metadata.exists() {
                    rerun_paths.push(metadata);
                }
            }
        }
    }
}

fn prune_mirror_tree(root: &Path, keep_files: &BTreeSet<PathBuf>) -> std::io::Result<()> {
    if !root.exists() {
        return Ok(());
    }

    prune_mirror_subtree(root, Path::new(""), keep_files)?;
    Ok(())
}

fn prune_mirror_subtree(
    root: &Path,
    relative: &Path,
    keep_files: &BTreeSet<PathBuf>,
) -> std::io::Result<bool> {
    let current_path = if relative.as_os_str().is_empty() {
        root.to_path_buf()
    } else {
        root.join(relative)
    };

    let mut has_required_descendants = false;
    let entries = match fs::read_dir(&current_path) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(true),
        Err(err) => return Err(err),
    };

    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name();
        let child_relative = if relative.as_os_str().is_empty() {
            PathBuf::from(&file_name)
        } else {
            relative.join(&file_name)
        };

        let file_type = entry.file_type()?;
        let entry_path = entry.path();
        if file_type.is_dir() {
            if prune_mirror_subtree(root, &child_relative, keep_files)? {
                fs::remove_dir_all(&entry_path)?;
            } else {
                has_required_descendants = true;
            }
        } else if keep_files.contains(&child_relative) {
            has_required_descendants = true;
        } else {
            fs::remove_file(&entry_path)?;
        }
    }

    Ok(!has_required_descendants && !relative.as_os_str().is_empty())
}

fn install_collection_asset(source: &Path, destination: &Path) -> std::io::Result<()> {
    if destination.exists() {
        if is_same_file(source, destination)? {
            return Ok(());
        }
        fs::remove_file(destination)?;
    }

    match fs::hard_link(source, destination) {
        Ok(_) => Ok(()),
        Err(err) => {
            if err.kind() == ErrorKind::AlreadyExists {
                Ok(())
            } else {
                fs::copy(source, destination).map(|_| ())
            }
        }
    }
}

type OfflineAssetTables = (String, String);

type OfflineEntryTables = (String, OfflineAssetTables);

type AssetMatchTables = (Vec<String>, Vec<String>);

fn render_collection_assets(
    asset_map: &BTreeMap<(String, String), AssetEntry>,
    mirror_prefix: &str,
) -> AssetMatchTables {
    let mut asset_definitions = Vec::new();
    let mut asset_match_entries = Vec::new();

    for entry in asset_map.values() {
        let mirror_path = format!(
            "{}/{}/{}",
            mirror_prefix.trim_end_matches('/'),
            entry.collection_id,
            entry.relative_path
        );
        let mirror_literal = serde_json::to_string(&mirror_path).unwrap();
        let collection_literal = serde_json::to_string(&entry.collection_id).unwrap();
        let relative_literal = serde_json::to_string(&entry.relative_path).unwrap();

        asset_definitions.push(format!(
            "static {}: Asset = dioxus::prelude::asset!({});",
            entry.const_name, mirror_literal
        ));
        asset_match_entries.push(format!(
            "        ({}, {}) => Some(&{}),",
            collection_literal, relative_literal, entry.const_name
        ));
    }

    (asset_definitions, asset_match_entries)
}

fn render_hero_match_section(hero_match_arms: &[String]) -> String {
    if hero_match_arms.is_empty() {
        "        _ => None,".to_string()
    } else {
        format!("{}\n        _ => None,", hero_match_arms.join("\n"))
    }
}

fn render_offline_entry_tables(
    layout: &OfflineProjectLayout,
    offline_entries: &[OfflineEntryRecord],
    asset_map: &BTreeMap<(String, String), AssetEntry>,
) -> OfflineEntryTables {
    let mut entry_assets_statics =
        vec!["static OFFLINE_EMPTY_ASSETS: [&str; 0] = [];".to_string()];
    let mut entry_match_arms = Vec::new();
    let mut used_idents = BTreeSet::new();

    for entry in offline_entries {
        let assets_ref = if entry.asset_paths.is_empty() {
            "OFFLINE_EMPTY_ASSETS".to_string()
        } else {
            let ident =
                sanitize_entry_ident(&entry.collection_id, &entry.entry_id, &mut used_idents);
            let asset_literals: Vec<String> = entry
                .asset_paths
                .iter()
                .map(|path| serde_json::to_string(path).unwrap())
                .collect();
            entry_assets_statics.push(format!(
                "static {ident}: [&str; {}] = [{}];",
                entry.asset_paths.len(),
                asset_literals.join(", ")
            ));
            ident
        };

        let body_literal = serde_json::to_string(&entry.body).unwrap();
        let collection_literal = serde_json::to_string(&entry.collection_id).unwrap();
        let entry_literal = serde_json::to_string(&entry.entry_id).unwrap();
        entry_match_arms.push(format!(
            "        ({}, {}) => Some(OfflineEntry {{ body: {}, assets: &{} }}),",
            collection_literal, entry_literal, body_literal, assets_ref
        ));
    }

    let entry_match_body = if entry_match_arms.is_empty() {
        "        _ => None,".to_string()
    } else {
        format!("{}\n        _ => None,", entry_match_arms.join("\n"))
    };

    let mut offline_asset_match_entries = Vec::new();
    for entry in asset_map.values() {
        let offline_path =
            make_offline_asset_path(layout, &entry.collection_id, &entry.relative_path);
        let literal = serde_json::to_string(&offline_path).unwrap();
        let collection_literal = serde_json::to_string(&entry.collection_id).unwrap();
        let relative_literal = serde_json::to_string(&entry.relative_path).unwrap();
        offline_asset_match_entries.push(format!(
            "        ({}, {}) => Some({}),",
            collection_literal, relative_literal, literal
        ));
    }

    let offline_asset_match_body = if offline_asset_match_entries.is_empty() {
        "        _ => None,".to_string()
    } else {
        format!(
            "{}\n        _ => None,",
            offline_asset_match_entries.join("\n")
        )
    };

    (
        entry_assets_statics.join("\n\n"),
        (entry_match_body, offline_asset_match_body),
    )
}

fn sanitize_entry_ident(
    collection_id: &str,
    entry_id: &str,
    used: &mut BTreeSet<String>,
) -> String {
    let mut base = format!("{}_{}", collection_id, entry_id)
        .to_uppercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>();

    while base.contains("__") {
        base = base.replace("__", "_");
    }

    if base.starts_with(|c: char| c.is_ascii_digit()) {
        base = format!("_{}", base);
    }

    let mut candidate = base.clone();
    let mut counter = 1;
    while used.contains(&candidate) {
        candidate = format!("{base}_{counter}");
        counter += 1;
    }

    used.insert(candidate.clone());
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    struct AllowAll;
    impl CollectionInclusion for AllowAll {
        fn is_included(&self, _collection_id: &str) -> bool {
            true
        }
    }

    fn layout() -> OfflineProjectLayout {
        OfflineProjectLayout {
            entry_assets_dir: "assets".into(),
            entry_markdown_file: "index.md".into(),
            collection_metadata_file: "collection.json".into(),
            excluded_dir_name: "prod".into(),
            excluded_path_fragment: "/prod/".into(),
            collection_asset_literal_prefix: "/content/programs".into(),
            offline_site_root: "site".into(),
            collections_dir_name: "programs".into(),
            offline_bundle_root: "target/offline-html".into(),
            index_html_file: "index.html".into(),
            target_dir: "target".into(),
            offline_manifest_json: "offline_manifest.json".into(),
        }
    }

    #[test]
    fn prune_mirror_tree_removes_stale_entries() -> std::io::Result<()> {
        let temp = tempdir()?;
        let root = temp.path();
        let mirror_root = root.join("mirror");

        fs::create_dir_all(mirror_root.join("program_a/assets"))?;
        fs::write(mirror_root.join("program_a/assets/keep.txt"), b"keep")?;
        fs::create_dir_all(mirror_root.join("program_a/tmp"))?;
        fs::write(mirror_root.join("program_a/tmp/unused.bin"), b"unused")?;
        fs::create_dir_all(mirror_root.join("program_b"))?;
        fs::write(mirror_root.join("program_b/stale.txt"), b"stale")?;

        let mut keep = BTreeSet::new();
        keep.insert(PathBuf::from("program_a/assets/keep.txt"));

        prune_mirror_tree(&mirror_root, &keep)?;

        assert!(mirror_root.join("program_a/assets/keep.txt").exists());
        assert!(!mirror_root.join("program_a/tmp").exists());
        assert!(!mirror_root.join("program_b").exists());

        Ok(())
    }

    #[test]
    fn install_collection_asset_reuses_existing_links() -> std::io::Result<()> {
        let temp = tempdir()?;
        let root = temp.path();

        let source_root = root.join("source");
        let mirror_root = root.join("mirror");
        fs::create_dir_all(&source_root)?;
        fs::create_dir_all(&mirror_root)?;

        let source = source_root.join("file.txt");
        fs::write(&source, b"content")?;
        let destination = mirror_root.join("file.txt");

        install_collection_asset(&source, &destination)?;
        assert!(destination.exists());
        assert!(same_file::is_same_file(&source, &destination)?);

        install_collection_asset(&source, &destination)?;
        assert!(same_file::is_same_file(&source, &destination)?);

        Ok(())
    }
}
