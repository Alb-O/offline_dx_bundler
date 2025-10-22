//! Generate the offline manifest by scanning authored content and assets.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::asset_paths::make_offline_asset_path;
use crate::builder::BuildResult;
use crate::config::load_document;
use crate::manifest::markdown::{
  collect_markdown_asset_references, extract_first_heading, parse_entry_markdown,
  parse_order_from_id, resolve_markdown_assets,
};
use crate::manifest::scanning::{collect_assets_recursively, sanitize_const_name};
use crate::models::{
  AssetEntry, CollectionCatalogRecord, CollectionMetaRecord, EntryRecord, ManifestGenerationResult,
  OfflineEntryRecord,
};
use crate::project::OfflineProjectLayout;
use crate::selection::CollectionInclusion;

/// Traverse the authored collections and build the intermediate offline manifest data structure.
pub fn generate_offline_manifest<S: CollectionInclusion>(
  layout: &OfflineProjectLayout,
  collections_dir: &Path,
  selection: &S,
) -> BuildResult<ManifestGenerationResult> {
  let mut hero_match_arms = Vec::new();
  let mut asset_map: BTreeMap<(String, String), AssetEntry> = BTreeMap::new();
  let mut used_names = BTreeSet::new();
  let mut collection_catalog: Vec<CollectionCatalogRecord> = Vec::new();
  let mut offline_entries: Vec<OfflineEntryRecord> = Vec::new();
  let mut hero_asset_paths: BTreeSet<String> = BTreeSet::new();

  if let Ok(entries) = fs::read_dir(collections_dir) {
    for entry in entries.flatten() {
      if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
        continue;
      }

      let collection_name = entry.file_name().to_string_lossy().to_string();
      if collection_name.starts_with('.') {
        continue;
      }

      let collection_path = entry.path();
      walk_collection_tree(
        layout,
        &collection_path,
        &collection_name,
        selection,
        &mut asset_map,
        &mut used_names,
        &mut hero_match_arms,
        &mut hero_asset_paths,
        &mut collection_catalog,
        &mut offline_entries,
      );
    }
  }

  Ok(ManifestGenerationResult {
    collection_catalog,
    offline_entries,
    asset_map,
    hero_asset_paths,
    hero_match_arms,
  })
}

fn walk_collection_tree<S: CollectionInclusion>(
  parent_layout: &OfflineProjectLayout,
  collection_path: &Path,
  collection_id: &str,
  selection: &S,
  asset_map: &mut BTreeMap<(String, String), AssetEntry>,
  used_names: &mut BTreeSet<String>,
  hero_match_arms: &mut Vec<String>,
  hero_asset_paths: &mut BTreeSet<String>,
  collection_catalog: &mut Vec<CollectionCatalogRecord>,
  offline_entries: &mut Vec<OfflineEntryRecord>,
) {
  let metadata_path = collection_path.join(&parent_layout.collection_metadata_file);
  let mut collection_layout = parent_layout.clone();
  let mut meta: Option<CollectionMetaRecord> = None;

  if let Some((payload, overrides)) = load_document(&metadata_path) {
    overrides.apply_to_layout(&mut collection_layout);
    meta = serde_json::from_value(payload).ok();
  }

  if let Some(meta) = meta
    && selection.is_included(collection_id)
  {
    collect_assets_recursively(
      collection_id,
      collection_path,
      Path::new(""),
      false,
      asset_map,
      used_names,
      &collection_layout.excluded_dir_name,
      &collection_layout.entry_assets_dir,
      &collection_layout.entry_markdown_file,
      &collection_layout.excluded_path_fragment,
      &collection_layout.collection_asset_literal_prefix,
      collection_layout.collection_metadata_file.as_str(),
    );

    if let Some(hero_image) = meta.hero_image.as_deref() {
      let hero_rel = hero_image.trim_start_matches('/').replace('\\', "/");
      if !hero_rel.is_empty() {
        asset_map
          .entry((collection_id.to_string(), hero_rel.clone()))
          .or_insert_with(|| {
            let const_name = sanitize_const_name(collection_id, &hero_rel, used_names);
            used_names.insert(const_name.clone());
            let asset_path = format!(
              "{}/{}/{}",
              collection_layout.collection_asset_literal_prefix.as_str(),
              collection_id,
              hero_rel
            );
            AssetEntry {
              const_name: const_name.clone(),
              literal_path: asset_path,
              collection_id: collection_id.to_string(),
              relative_path: hero_rel.clone(),
            }
          });

        if let Some(entry) = asset_map.get(&(collection_id.to_string(), hero_rel.clone())) {
          let collection_literal = serde_json::to_string(collection_id).unwrap();
          hero_match_arms.push(format!(
            "        {} => Some(&{}),",
            collection_literal, entry.const_name
          ));
          hero_asset_paths.insert(make_offline_asset_path(
            &collection_layout,
            &entry.collection_id,
            &entry.relative_path,
          ));
        }
      }
    }

    let mut entry_records: Vec<(usize, EntryRecord)> = Vec::new();

    if let Ok(entry_iter) = fs::read_dir(collection_path) {
      for entry_dir in entry_iter.flatten() {
        let entry_path = entry_dir.path();

        if !entry_path.is_dir() {
          continue;
        }

        let entry_id = entry_dir.file_name().to_string_lossy().to_string();

        if entry_id.starts_with('.') || entry_id == collection_layout.entry_assets_dir {
          continue;
        }

        let markdown_path = entry_path.join(&collection_layout.entry_markdown_file);
        if !markdown_path.exists() {
          continue;
        }

        if let Some((frontmatter, body)) = parse_entry_markdown(&markdown_path) {
          let entry_title = frontmatter
            .title
            .clone()
            .or_else(|| extract_first_heading(&body))
            .unwrap_or_else(|| entry_id.clone());

          let order = frontmatter
            .order
            .or_else(|| parse_order_from_id(&entry_id))
            .unwrap_or(usize::MAX);

          let asset_slug = meta.asset_slug.as_deref();

          let references = collect_markdown_asset_references(&body);
          let (resolved_assets, unresolved_assets) = resolve_markdown_assets(
            &collection_layout,
            &references,
            asset_map,
            collection_id,
            &entry_id,
            asset_slug,
          );

          if !unresolved_assets.is_empty() {
            for unresolved in unresolved_assets {
              println!(
                "cargo:warning=Unresolved offline asset reference '{}' in {}/{}",
                unresolved, collection_id, entry_id
              );
            }
          }

          offline_entries.push(OfflineEntryRecord {
            collection_id: collection_id.to_string(),
            entry_id: entry_id.clone(),
            body: body.clone(),
            asset_paths: resolved_assets,
          });

          entry_records.push((order, EntryRecord {
            id: entry_id.clone(),
            title: entry_title,
            section: frontmatter.section.clone(),
            sequence: order,
            source: format!(
              "{}/{}/{}",
              collection_id, entry_id, collection_layout.entry_markdown_file
            ),
          }));
        }
      }
    }

    entry_records.sort_by(|(order_a, entry_a), (order_b, entry_b)| {
      order_a
        .cmp(order_b)
        .then_with(|| entry_a.id.cmp(&entry_b.id))
    });

    let entries: Vec<EntryRecord> = entry_records
      .into_iter()
      .enumerate()
      .map(|(index, (_, mut entry))| {
        entry.sequence = index + 1;
        entry
      })
      .collect();

    collection_catalog.push(CollectionCatalogRecord {
      id: collection_id.to_string(),
      meta,
      entries,
    });
  }

  if let Ok(children) = fs::read_dir(collection_path) {
    for child in children.flatten() {
      if !child.file_type().is_ok_and(|ft| ft.is_dir()) {
        continue;
      }

      let name = child.file_name().to_string_lossy().to_string();
      if name.starts_with('.') {
        continue;
      }

      let child_path = child.path();
      if !child_path
        .join(&collection_layout.collection_metadata_file)
        .exists()
      {
        continue;
      }

      let child_id = if collection_id.is_empty() {
        name.clone()
      } else {
        format!("{}/{}", collection_id, name)
      };

      walk_collection_tree(
        &collection_layout,
        &child_path,
        &child_id,
        selection,
        asset_map,
        used_names,
        hero_match_arms,
        hero_asset_paths,
        collection_catalog,
        offline_entries,
      );
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::project::OfflineProjectLayout;
  use crate::selection::CollectionInclusion;
  use tempfile::tempdir;

  impl CollectionInclusion for () {
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

  fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
  }

  #[test]
  fn generates_catalog_and_offline_entries() {
    let dir = tempdir().unwrap();
    let collections_dir = dir.path();

    let collection_dir = collections_dir.join("p001-intro");
    let _ = fs::create_dir_all(collection_dir.join("assets"));

    write_file(
      &collection_dir.join("collection.json"),
      r#"{"title":"Intro","assetSlug":"intro","heroImage":"/assets/cover.png"}"#,
    );
    write_file(&collection_dir.join("assets/cover.png"), "hero");
    write_file(
      &collection_dir.join("001-welcome/index.md"),
      "---\ntitle: Welcome\n---\n![Alt](image.png)\n",
    );
    write_file(
      &collection_dir.join("001-welcome/assets/image.png"),
      "image",
    );

    let layout = layout();
    let selection = ();
    let result = generate_offline_manifest(&layout, collections_dir, &selection).unwrap();

    assert_eq!(result.collection_catalog.len(), 1);
    let collection = &result.collection_catalog[0];
    assert_eq!(collection.id, "p001-intro");
    assert_eq!(collection.entries.len(), 1);
    assert_eq!(collection.entries[0].id, "001-welcome");
    assert_eq!(collection.entries[0].sequence, 1);

    assert_eq!(result.offline_entries.len(), 1);
    let offline = &result.offline_entries[0];
    assert_eq!(offline.collection_id, "p001-intro");
    assert_eq!(offline.entry_id, "001-welcome");
    assert_eq!(offline.asset_paths.len(), 1);

    assert!(
      result
        .asset_map
        .contains_key(&("p001-intro".into(), "assets/image.png".into()))
    );
    assert!(
      result
        .hero_asset_paths
        .contains("programs/p001-intro/assets/cover.png")
    );
    assert!(!result.hero_match_arms.is_empty());
  }
}
