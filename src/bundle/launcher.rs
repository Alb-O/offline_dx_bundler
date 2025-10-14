//! Generate a tiny launcher HTML file for offline bundles with nested site roots.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::project::OfflineProjectLayout;

/// Write a root `index.html` that redirects into the bundled site when required.
pub fn write_root_launcher(
  layout: &OfflineProjectLayout,
  root_dir: &Path,
  site_prefix: &str,
) -> Result<()> {
  fs::create_dir_all(root_dir)
    .with_context(|| format!("failed to create {}", root_dir.display()))?;

  let trimmed_prefix = site_prefix.trim_matches('/');
  if trimmed_prefix.is_empty() {
    return Ok(());
  }

  let target = root_dir.join(layout.index_html_file);
  let redirect_target = format!("{}/{}", trimmed_prefix, layout.index_html_file);
  let html = format!(
    r#"<!DOCTYPE html>
<html>
  <head>
    <meta charset=\"utf-8\">
    <title>Offline Bundle</title>
    <meta http-equiv=\"refresh\" content=\"0;url={redirect}\">
    <script>
      (function () {{
        var target = "{redirect}";
        if (typeof window !== "undefined" && window.location) {{
          if (typeof window.location.replace === "function") {{
            window.location.replace(target);
          }} else {{
            window.location.href = target;
          }}
        }}
      }})();
    </script>
  </head>
  <body>
    <p>Redirecting to <a href=\"{redirect}\">{redirect}</a>...</p>
  </body>
</html>
"#,
    redirect = redirect_target
  );
  fs::write(&target, html).with_context(|| format!("failed to write {}", target.display()))
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
  fn writes_redirect_launcher() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("offline");
    write_root_launcher(&layout(), &root, "site").unwrap();

    let index_path = root.join("index.html");
    assert!(index_path.exists());
    let content = fs::read_to_string(index_path).unwrap();
    assert!(content.contains("site/index.html"));
    assert!(content.contains("window.location.replace"));
    assert!(content.contains("window.location.href"));
  }

  #[test]
  fn skips_redirect_when_site_is_root() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("offline");
    fs::create_dir_all(&root).unwrap();
    let index_path = root.join("index.html");
    fs::write(&index_path, "original").unwrap();

    write_root_launcher(&layout(), &root, "").unwrap();

    let content = fs::read_to_string(index_path).unwrap();
    assert_eq!(content, "original");
  }
}
