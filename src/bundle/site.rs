//! HTML patching utilities for the offline bundle site output.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use regex::Regex;

use crate::project::OfflineProjectLayout;

const INLINE_LOADER_TEMPLATE: &str = r#"    <script>
      window.addEventListener('DOMContentLoaded', () => {
        if (!window.location.hash) {
          window.location.replace('#/');
        }
        const init = window.__dx_mainInit;
        if (!init) {
          console.error('Offline loader could not find Dioxus bootstrap.');
          return;
        }
        const wasmBytes = window.__pivotOfflineWasm;
        init(wasmBytes).catch((err) => {
          console.error('Failed to launch offline bundle', err);
        });
      });
    </script>
"#;

/// Update the generated `index.html` to load JavaScript and WebAssembly without a module loader.
pub fn patch_site_index(
    layout: &OfflineProjectLayout,
    site_root: &Path,
) -> Result<(String, String)> {
    let index_path = site_root.join(layout.index_html_file);
    let mut text = fs::read_to_string(&index_path)
        .with_context(|| format!("failed to read {}", index_path.display()))?;

    let assets_prefix = format!("{}/", layout.entry_assets_dir);
    text = text.replace(&format!("/./{}", assets_prefix), &assets_prefix);

    let escaped_assets_prefix = regex::escape(&assets_prefix);
    let script_pattern = Regex::new(&format!(
        r#"(?i)<script[^>]*type="module"[^>]*src="{}([^"]+\.js)"[^>]*></script>"#,
        escaped_assets_prefix
    ))
    .expect("invalid script regex");
    let script_caps = script_pattern
        .captures(&text)
        .ok_or_else(|| anyhow!("failed to locate module script tag in offline index.html"))?;
    let js_name = script_caps
        .get(1)
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| anyhow!("failed to extract JS module name"))?;

    let wasm_pattern = Regex::new(&format!(
        r#"(?i)href="{}([^"]+\.wasm)""#,
        escaped_assets_prefix
    ))
    .expect("invalid wasm regex");
    let wasm_caps = wasm_pattern
        .captures(&text)
        .ok_or_else(|| anyhow!("failed to locate wasm preload reference in offline index.html"))?;
    let wasm_name = wasm_caps
        .get(1)
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| anyhow!("failed to extract wasm module name"))?;

    let escaped_assets_dir = regex::escape(layout.entry_assets_dir);
    let preload_pattern = Regex::new(&format!(
        r#"(?i)<link[^>]*rel="preload"[^>]*{}/[^>]+>"#,
        escaped_assets_dir
    ))
    .expect("invalid preload regex");
    text = preload_pattern.replace_all(&text, "").into_owned();

    let replacement = format!(
        "<script defer src=\"{prefix}{js}\"></script>\n{loader}",
        prefix = assets_prefix,
        js = js_name,
        loader = INLINE_LOADER_TEMPLATE
    );
    text = script_pattern
        .replace_all(&text, replacement.as_str())
        .into_owned();

    let crossorigin_pattern = Regex::new(r"\s+crossorigin").expect("invalid crossorigin regex");
    text = crossorigin_pattern.replace_all(&text, "").into_owned();

    fs::write(&index_path, &text)
        .with_context(|| format!("failed to write {}", index_path.display()))?;

    Ok((js_name, wasm_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn layout() -> OfflineProjectLayout<'static> {
        OfflineProjectLayout {
            entry_assets_dir: "assets",
            entry_markdown_file: "index.md",
            collection_metadata_file: "program.json",
            excluded_dir_name: "prod",
            excluded_path_fragment: "/prod/",
            collection_asset_literal_prefix: "/content/programs",
            offline_site_root: "site",
            collections_dir_name: "programs",
            offline_bundle_root: "target/offline-html",
            index_html_file: "index.html",
            target_dir: "target",
            offline_manifest_json: "offline_manifest.json",
        }
    }

    #[test]
    fn patches_index_and_returns_asset_names() {
        let dir = tempdir().unwrap();
        let layout = layout();
        let index_path = dir.path().join(layout.index_html_file);
        let original = r#"
      <html>
        <head>
          <link rel="preload" href="/./assets/module_bg.wasm" as="fetch" crossorigin>
        </head>
        <body>
          <script type="module" src="/./assets/module.js" crossorigin></script>
        </body>
      </html>
    "#;
        fs::write(&index_path, original).unwrap();

        let (js_name, wasm_name) = patch_site_index(&layout, dir.path()).unwrap();
        assert_eq!(js_name, "module.js");
        assert_eq!(wasm_name, "module_bg.wasm");

        let updated = fs::read_to_string(&index_path).unwrap();
        assert!(updated.contains("window.addEventListener('DOMContentLoaded'"));
        assert!(!updated.contains("crossorigin"));
        assert!(updated.contains("<script defer src=\"assets/module.js\"></script>"));
    }
}
