//! Stylesheet helpers ensuring predictable filenames in the offline bundle.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

use crate::project::OfflineProjectLayout;

/// Ensure deterministic stylesheet names are available for the offline launcher.
pub fn ensure_stylesheet_aliases(
  layout: &OfflineProjectLayout,
  site_root: &Path,
) -> Result<()> {
  ensure_tailwind_alias(layout, site_root)?;
  Ok(())
}

fn ensure_tailwind_alias(layout: &OfflineProjectLayout, site_root: &Path) -> Result<()> {
  let target = site_root.join("tailwind.css");
  if target.exists() {
    return Ok(());
  }

  let assets_dir = site_root.join(layout.module_assets_dir);
  let Some(source) = find_hashed_stylesheet(&assets_dir, "tailwind")? else {
    return Err(anyhow!(
      "failed to locate hashed tailwind stylesheet in {}",
      assets_dir.display()
    ));
  };

  let effective_source = resolve_tailwind_source(layout, &source)?;

  fs::copy(&effective_source, &target).with_context(|| {
    format!(
      "failed to copy {} to {}",
      effective_source.display(),
      target.display()
    )
  })?;

  Ok(())
}

fn find_hashed_stylesheet(assets_dir: &Path, stem: &str) -> Result<Option<PathBuf>> {
  if !assets_dir.is_dir() {
    return Ok(None);
  }

  let prefix = format!("{stem}-");
  let mut matches: Vec<PathBuf> = Vec::new();

  for entry in fs::read_dir(assets_dir).with_context(|| {
    format!(
      "failed to read assets directory at {}",
      assets_dir.display()
    )
  })? {
    let entry = entry?;
    if !entry.file_type()?.is_file() {
      continue;
    }

    let file_name = entry.file_name();
    let Some(name) = file_name.to_str() else {
      continue;
    };

    if name.starts_with(&prefix) && name.ends_with(".css") {
      matches.push(entry.path());
    }
  }

  matches.sort();
  Ok(matches.pop())
}

fn resolve_tailwind_source(
  layout: &OfflineProjectLayout,
  default: &Path,
) -> Result<PathBuf> {
  if is_compiled_tailwind(default)? {
    return Ok(default.to_path_buf());
  }

  if let Some(fallback) = find_debug_tailwind_stylesheet(layout)?
    && is_compiled_tailwind(&fallback)?
  {
    fs::copy(&fallback, default).with_context(|| {
      format!(
        "failed to copy fallback tailwind stylesheet {} to {}",
        fallback.display(),
        default.display()
      )
    })?;
    return Ok(default.to_path_buf());
  }

  Ok(default.to_path_buf())
}

fn is_compiled_tailwind(path: &Path) -> Result<bool> {
  let content = fs::read_to_string(path)
    .with_context(|| format!("failed to read stylesheet at {}", path.display()))?;

  Ok(!content.contains("@import \"tailwindcss\"") && !content.contains("@apply"))
}

fn find_debug_tailwind_stylesheet(
  layout: &OfflineProjectLayout,
) -> Result<Option<PathBuf>> {
  let dx_root = Path::new(layout.target_dir).join("dx");
  if !dx_root.is_dir() {
    return Ok(None);
  }

  for app_dir in
    fs::read_dir(&dx_root).with_context(|| format!("failed to read {}", dx_root.display()))?
  {
    let app_dir = app_dir?;
    let debug_tailwind = app_dir
      .path()
      .join("debug")
      .join("web")
      .join("public")
      .join("tailwind.css");
    if debug_tailwind.exists() {
      return Ok(Some(debug_tailwind));
    }
  }

  Ok(None)
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::tempdir;

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

  #[test]
  fn finds_latest_hashed_stylesheet() {
    let dir = tempdir().unwrap();
    let assets_dir = dir.path();
    fs::create_dir_all(assets_dir).unwrap();

    let older = assets_dir.join("tailwind-111.css");
    fs::write(&older, "old").unwrap();
    let newer = assets_dir.join("tailwind-222.css");
    fs::write(&newer, "new").unwrap();

    let result = find_hashed_stylesheet(assets_dir, "tailwind").unwrap();
    assert_eq!(result.unwrap(), newer);
  }

  #[test]
  fn detects_uncompiled_tailwind() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("tailwind.css");
    fs::write(&file, "@import \"tailwindcss\";").unwrap();

    let compiled = is_compiled_tailwind(&file).unwrap();
    assert!(!compiled);
  }
}
