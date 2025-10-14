//! Generate the offline manifest by scanning authored content and assets.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use crate::asset_paths::make_offline_asset_path;
use crate::builder::BuildResult;
use crate::manifest::markdown::{
  collect_markdown_asset_references, extract_first_heading, parse_module_markdown,
  parse_order_from_id, resolve_markdown_assets,
};
use crate::manifest::scanning::{collect_assets_recursively, sanitize_const_name};
use crate::models::{
  AssetEntry, ManifestGenerationResult, ModuleRecord, OfflineModuleRecord, ProgramCatalogRecord,
  ProgramMetaRecord,
};
use crate::project::OfflineProjectLayout;
use crate::selection::ProgramInclusion;

/// Traverse the authored programs and build the intermediate offline manifest data structure.
pub fn generate_offline_manifest<S: ProgramInclusion>(
  layout: &OfflineProjectLayout,
  programs_dir: &Path,
  selection: &S,
) -> BuildResult<ManifestGenerationResult> {
  let mut hero_match_arms = Vec::new();
  let mut asset_map: BTreeMap<(String, String), AssetEntry> = BTreeMap::new();
  let mut used_names = BTreeSet::new();
  let mut program_catalog: Vec<ProgramCatalogRecord> = Vec::new();
  let mut offline_modules: Vec<OfflineModuleRecord> = Vec::new();
  let mut hero_asset_paths: BTreeSet<String> = BTreeSet::new();

  if let Ok(entries) = fs::read_dir(programs_dir) {
    for entry in entries.flatten() {
      if !entry.file_type().is_ok_and(|ft| ft.is_dir()) {
        continue;
      }

      let program_id = entry.file_name().to_string_lossy().to_string();
      if program_id.starts_with('.') {
        continue;
      }

      if !selection.is_included(&program_id) {
        continue;
      }

      let program_json_path = entry.path().join(layout.program_metadata_file);
      if !program_json_path.exists() {
        continue;
      }

      let json_content = match fs::read_to_string(&program_json_path) {
        Ok(content) => content,
        Err(_) => continue,
      };

      let meta: ProgramMetaRecord = match serde_json::from_str(&json_content) {
        Ok(meta) => meta,
        Err(_) => continue,
      };

      let program_path = entry.path();

      collect_assets_recursively(
        &program_id,
        &program_path,
        Path::new(""),
        false,
        &mut asset_map,
        &mut used_names,
        layout.prod_dir_name,
        layout.module_assets_dir,
        layout.module_markdown_file,
        layout.prod_path_fragment,
        layout.program_asset_literal_prefix,
        layout.program_metadata_file,
      );

      if let Some(hero_image) = meta.hero_image.as_deref() {
        let hero_rel = hero_image.trim_start_matches('/').replace('\\', "/");
        if !hero_rel.is_empty() {
          asset_map
            .entry((program_id.clone(), hero_rel.clone()))
            .or_insert_with(|| {
              let const_name = sanitize_const_name(&program_id, &hero_rel, &used_names);
              used_names.insert(const_name.clone());
              let asset_path = format!(
                "{}/{}/{}",
                layout.program_asset_literal_prefix, program_id, hero_rel
              );
              AssetEntry {
                const_name: const_name.clone(),
                literal_path: asset_path,
                program_id: program_id.clone(),
                relative_path: hero_rel.clone(),
              }
            });

          if let Some(entry) = asset_map.get(&(program_id.clone(), hero_rel.clone())) {
            let program_literal = serde_json::to_string(&program_id).unwrap();
            hero_match_arms.push(format!(
              "        {} => Some(&{}),",
              program_literal, entry.const_name
            ));
            hero_asset_paths.insert(make_offline_asset_path(
              layout,
              &entry.program_id,
              &entry.relative_path,
            ));
          }
        }
      }

      let mut module_entries: Vec<(usize, ModuleRecord)> = Vec::new();

      if let Ok(module_iter) = fs::read_dir(&program_path) {
        for module_entry in module_iter.flatten() {
          let module_path = module_entry.path();

          if !module_path.is_dir() {
            continue;
          }

          let module_id = module_entry.file_name().to_string_lossy().to_string();

          if module_id.starts_with('.') || module_id == layout.module_assets_dir {
            continue;
          }

          let markdown_path = module_path.join(layout.module_markdown_file);
          if !markdown_path.exists() {
            continue;
          }

          if let Some((frontmatter, body)) = parse_module_markdown(&markdown_path) {
            let module_title = frontmatter
              .title
              .clone()
              .or_else(|| extract_first_heading(&body))
              .unwrap_or_else(|| module_id.clone());

            let order = frontmatter
              .order
              .or_else(|| parse_order_from_id(&module_id))
              .unwrap_or(usize::MAX);

            let asset_slug = meta.asset_slug.as_deref();

            let references = collect_markdown_asset_references(&body);
            let (resolved_assets, unresolved_assets) = resolve_markdown_assets(
              layout,
              &references,
              &asset_map,
              &program_id,
              &module_id,
              asset_slug,
            );

            if !unresolved_assets.is_empty() {
              for unresolved in unresolved_assets {
                println!(
                  "cargo:warning=Unresolved offline asset reference '{}' in {}/{}",
                  unresolved, program_id, module_id
                );
              }
            }

            offline_modules.push(OfflineModuleRecord {
              program_id: program_id.clone(),
              module_id: module_id.clone(),
              body: body.clone(),
              asset_paths: resolved_assets,
            });

            module_entries.push((
              order,
              ModuleRecord {
                id: module_id.clone(),
                title: module_title,
                section: frontmatter.section.clone(),
                sequence: order,
                source: format!("{}/{}/{}", program_id, module_id, layout.module_markdown_file),
              },
            ));
          }
        }
      }

      module_entries.sort_by(|(order_a, module_a), (order_b, module_b)| {
        order_a
          .cmp(order_b)
          .then_with(|| module_a.id.cmp(&module_b.id))
      });

      let modules: Vec<ModuleRecord> = module_entries
        .into_iter()
        .enumerate()
        .map(|(index, (_, mut module))| {
          module.sequence = index + 1;
          module
        })
        .collect();

      program_catalog.push(ProgramCatalogRecord {
        id: program_id,
        meta,
        modules,
      });
    }
  }

  Ok(ManifestGenerationResult {
    program_catalog,
    offline_modules,
    asset_map,
    hero_asset_paths,
    hero_match_arms,
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::project::OfflineProjectLayout;
  use crate::selection::ProgramInclusion;
  use tempfile::tempdir;

  impl ProgramInclusion for () {
    fn is_included(&self, _program_id: &str) -> bool {
      true
    }
  }

  fn layout() -> OfflineProjectLayout<'static> {
    OfflineProjectLayout {
      module_assets_dir: "assets",
      module_markdown_file: "index.md",
      program_metadata_file: "program.json",
      prod_dir_name: "prod",
      prod_path_fragment: "/prod/",
      program_asset_literal_prefix: "/content/programs",
      offline_site_root: "site",
      programs_dir_name: "programs",
      offline_bundle_root: "target/offline-html",
      index_html_file: "index.html",
      target_dir: "target",
      offline_manifest_json: "offline_manifest.json",
    }
  }

  fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
  }

  #[test]
  fn generates_catalog_and_offline_modules() {
    let dir = tempdir().unwrap();
    let programs_dir = dir.path();

    let program_dir = programs_dir.join("p001-intro");
    fs::create_dir_all(program_dir.join("assets"));

    write_file(
      &program_dir.join("program.json"),
      r#"{"title":"Intro","assetSlug":"intro","heroImage":"/assets/cover.png"}"#,
    );
    write_file(&program_dir.join("assets/cover.png"), "hero");
    write_file(
      &program_dir.join("001-welcome/index.md"),
      "---\ntitle: Welcome\n---\n![Alt](image.png)\n",
    );
    write_file(&program_dir.join("001-welcome/assets/image.png"), "image");

    let layout = layout();
    let selection = ();
    let result = generate_offline_manifest(&layout, programs_dir, &selection).unwrap();

    assert_eq!(result.program_catalog.len(), 1);
    let program = &result.program_catalog[0];
    assert_eq!(program.id, "p001-intro");
    assert_eq!(program.modules.len(), 1);
    assert_eq!(program.modules[0].id, "001-welcome");
    assert_eq!(program.modules[0].sequence, 1);

    assert_eq!(result.offline_modules.len(), 1);
    let offline = &result.offline_modules[0];
    assert_eq!(offline.program_id, "p001-intro");
    assert_eq!(offline.module_id, "001-welcome");
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
