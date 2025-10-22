//! Directory scanning utilities for harvesting authored assets.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::models::{AssetEntry, AssetScanningConfig};

/// Walk the collection directory collecting asset entries and generated constant names.
pub fn collect_assets_recursively(
  collection_id: &str,
  dir: &Path,
  relative_root: &Path,
  in_assets_tree: bool,
  asset_map: &mut BTreeMap<(String, String), AssetEntry>,
  used_names: &mut BTreeSet<String>,
  config: &AssetScanningConfig,
) {
  if let Ok(entries) = fs::read_dir(dir) {
    for entry in entries.flatten() {
      let file_name = entry.file_name();
      let name_str = file_name.to_string_lossy();
      if name_str.starts_with('.') {
        continue;
      }

      let path = entry.path();
      if let Ok(file_type) = entry.file_type() {
        let mut next_relative = PathBuf::from(relative_root);
        if !relative_root.as_os_str().is_empty() {
          next_relative.push(&file_name);
        } else {
          next_relative = PathBuf::from(&file_name);
        }

        if file_type.is_dir() {
          if in_assets_tree && name_str == config.excluded_dir_name {
            continue;
          }
          let next_in_assets = in_assets_tree || name_str == config.entry_assets_dir;
          collect_assets_recursively(
            collection_id,
            &path,
            &next_relative,
            next_in_assets,
            asset_map,
            used_names,
            config,
          );
        } else if file_type.is_file()
          && (in_assets_tree
            || name_str == config.entry_markdown_file
            || name_str == config.collection_metadata_file)
        {
          let rel_path_str = next_relative.to_string_lossy().replace('\\', "/");

          if rel_path_str.contains(config.excluded_path_fragment) {
            continue;
          }

          let key = (collection_id.to_string(), rel_path_str.clone());
          if asset_map.contains_key(&key) {
            continue;
          }

          let const_name = sanitize_const_name(collection_id, &rel_path_str, used_names);
          used_names.insert(const_name.clone());
          let literal_path = format!(
            "{}/{}/{}",
            config.collection_asset_literal_prefix, collection_id, rel_path_str
          );

          asset_map.insert(key, AssetEntry {
            const_name,
            literal_path,
            collection_id: collection_id.to_string(),
            relative_path: rel_path_str,
          });
        }
      }
    }
  }
}

/// Generate a valid Rust identifier for a collection asset, deduplicating collisions.
pub fn sanitize_const_name(
  collection_id: &str,
  relative_path: &str,
  used: &BTreeSet<String>,
) -> String {
  let mut base = format!("{}_{}", collection_id, relative_path)
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

  candidate
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::tempdir;

  #[test]
  fn sanitizes_and_deduplicates_constant_names() {
    let mut used = BTreeSet::new();
    let name_one = sanitize_const_name("collection", "assets/file-name.png", &used);
    used.insert(name_one.clone());
    let name_two = sanitize_const_name("collection", "assets/file name.png", &used);
    assert_ne!(name_one, name_two);
    assert!(name_one.starts_with("COLLECTION_ASSETS"));
    assert!(name_two.ends_with("_1"));
  }

  #[test]
  fn collects_asset_entries_recursively() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let collection_dir = root.join("collection");
    let _ = fs::create_dir_all(collection_dir.join("entries/entry-one/assets"));

    fs::write(collection_dir.join("collection.json"), "{}").unwrap();
    fs::write(collection_dir.join("entries/entry-one/index.md"), "content").unwrap();
    fs::write(
      collection_dir.join("entries/entry-one/assets/image.png"),
      "binary",
    )
    .unwrap();

    let mut asset_map = BTreeMap::new();
    let mut used_names = BTreeSet::new();
    let config = AssetScanningConfig {
      excluded_dir_name: "prod",
      entry_assets_dir: "assets",
      entry_markdown_file: "index.md",
      excluded_path_fragment: "/prod/",
      collection_asset_literal_prefix: "/content/programs",
      collection_metadata_file: "collection.json",
    };

    collect_assets_recursively(
      "collection",
      &collection_dir,
      Path::new(""),
      false,
      &mut asset_map,
      &mut used_names,
      &config,
    );

    assert!(asset_map.contains_key(&("collection".into(), "collection.json".into())));
    assert!(asset_map.contains_key(&("collection".into(), "entries/entry-one/index.md".into())));
    assert!(asset_map.contains_key(&(
      "collection".into(),
      "entries/entry-one/assets/image.png".into()
    )));
  }
}
