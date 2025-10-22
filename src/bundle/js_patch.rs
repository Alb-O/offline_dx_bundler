//! Mutations applied to the generated JavaScript bootstrap for offline use.

use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, anyhow};
use base64::{Engine as _, engine::general_purpose};
use regex::Regex;
use serde_json::Value;

use crate::project::OfflineProjectLayout;

/// Patch the generated JavaScript module so it can bootstrap without a network request.
pub fn patch_js_module<F>(
  layout: &OfflineProjectLayout,
  site_root: &Path,
  js_name: &str,
  wasm_name: &str,
  resolve_binary_name: F,
) -> Result<()>
where
  F: FnOnce() -> Result<String>,
{
  let js_path = site_root.join(layout.entry_assets_dir()).join(js_name);
  let mut text = fs::read_to_string(&js_path)
    .with_context(|| format!("failed to read {}", js_path.display()))?;

  let assets_prefix = format!("{}/", layout.entry_assets_dir());
  text = text.replace(
    &format!("\"/./{}", assets_prefix),
    &format!("\"{}", assets_prefix),
  );

  let export_pattern = Regex::new(r"export\{[^}]+\};?$").expect("invalid export regex");
  text = export_pattern.replace_all(&text, "").into_owned();

  let import_meta_pattern =
    Regex::new(r#"const importMeta=\{url:"[^"]+",main:import\.meta\.main\};"#)
      .expect("invalid importMeta regex");
  let import_meta_replacement = "const __offlineScript=document.currentScript;\
const importMeta={url:__offlineScript?__offlineScript.src:window.location.href,main:false};";
  text = import_meta_pattern
    .replace(&text, import_meta_replacement)
    .into_owned();

  let wasm_path = site_root.join(layout.entry_assets_dir()).join(wasm_name);
  let wasm_bytes =
    fs::read(&wasm_path).with_context(|| format!("failed to read {}", wasm_path.display()))?;
  let wasm_base64 = general_purpose::STANDARD.encode(wasm_bytes);

  let decoder_snippet = format!(
    "const __offlineWasmBytes=(function(){{const binary=atob('{encoded}');\
const length=binary.length;const bytes=new Uint8Array(length);\
for(let i=0;i<length;i++){{bytes[i]=binary.charCodeAt(i);}}\
return bytes;}})();window.__pivotOfflineWasm=__offlineWasmBytes;\
globalThis.__pivotOfflineWasm=__offlineWasmBytes;",
    encoded = wasm_base64,
  );
  text = text.replace(
    "let wasm;",
    format!("let wasm;{decoder}", decoder = decoder_snippet).as_str(),
  );

  let binary_name = resolve_binary_name()?;
  let wasm_url_pattern = Regex::new(&format!(
    r#"new URL\("{}_bg\.wasm",importMeta\.url\)"#,
    regex::escape(&binary_name)
  ))
  .expect("invalid wasm URL regex");
  text = wasm_url_pattern
    .replace_all(&text, "__offlineWasmBytes")
    .into_owned();

  let bootstrap_pattern = Regex::new(
    r#"(?s)(?:window\.|globalThis\.)?__wasm_split_main_initSync=initSync;__wbg_init\(\{module_or_path:"[^"]+"\}\)\.then\(wasm=>\{.*\}\);"#,
  )
  .expect("invalid bootstrap regex");
  let bootstrap_replacement = "const __offlineInit=(bytes=__offlineWasmBytes)=>__wbg_init({module_or_path:bytes,module:bytes}).then(wasm=>{\
window.__dx_mainWasm=wasm;globalThis.__dx_mainWasm=wasm;if(wasm.__wbindgen_start===undefined){wasm.main();}return wasm;});\
window.__wasm_split_main_initSync=initSync;globalThis.__wasm_split_main_initSync=initSync;\
window.__dx___wbg_get_imports=__wbg_get_imports;globalThis.__dx___wbg_get_imports=__wbg_get_imports;\
window.__dx_mainInitSync=initSync;globalThis.__dx_mainInitSync=initSync;window.__dx_mainInit=__offlineInit;\
globalThis.__dx_mainInit=__offlineInit;";
  text = bootstrap_pattern
    .replace_all(&text, bootstrap_replacement)
    .into_owned();

  fs::write(&js_path, text).with_context(|| format!("failed to write {}", js_path.display()))?;

  Ok(())
}

/// Determine the primary binary target name from `cargo metadata`.
pub fn find_binary_name() -> Result<String> {
  let output = Command::new("cargo")
    .args(["metadata", "--no-deps", "--format-version", "1"])
    .output()
    .context("failed to run `cargo metadata`")?;

  if !output.status.success() {
    return Err(anyhow!(
      "`cargo metadata` failed with status {}",
      output.status
    ));
  }

  let metadata: Value =
    serde_json::from_slice(&output.stdout).context("failed to parse cargo metadata JSON")?;
  let packages = metadata
    .get("packages")
    .and_then(|value| value.as_array())
    .ok_or_else(|| anyhow!("missing `packages` field in cargo metadata"))?;

  for package in packages {
    if let Some(targets) = package.get("targets").and_then(|value| value.as_array()) {
      for target in targets {
        let is_bin = target
          .get("kind")
          .and_then(|value| value.as_array())
          .map(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("bin")))
          .unwrap_or(false);
        if is_bin && let Some(name) = target.get("name").and_then(|value| value.as_str()) {
          return Ok(name.to_string());
        }
      }
    }
  }

  Err(anyhow!("No binary target found"))
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::tempdir;

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
  fn patches_js_module_with_injected_binary_name() {
    let dir = tempdir().unwrap();
    let layout = layout();
    let assets_dir = dir.path().join(layout.entry_assets_dir());
    fs::create_dir_all(&assets_dir).unwrap();

    let js_path = assets_dir.join("module.js");
    let original_js = "let wasm;\nconst importMeta={url:\"/./assets/module.js\",main:import.meta.main};\nfunction boot() {\n  new URL(\"module_bg.wasm\",importMeta.url);\n}\nwindow.__wasm_split_main_initSync=initSync;__wbg_init({module_or_path:\"module_bg.wasm\"}).then(wasm=>{wasm.main();});\nexport{initSync};\n";
    fs::write(&js_path, original_js).unwrap();

    let wasm_path = assets_dir.join("module_bg.wasm");
    fs::write(&wasm_path, [0u8, 1, 2]).unwrap();

    patch_js_module(&layout, dir.path(), "module.js", "module_bg.wasm", || {
      Ok("module".into())
    })
    .unwrap();

    let updated = fs::read_to_string(&js_path).unwrap();
    assert!(updated.contains("window.__dx_mainInit"));
    assert!(updated.contains("globalThis.__dx_mainInit"));
    assert!(updated.contains("__offlineWasmBytes"));
    assert!(updated.contains("globalThis.__pivotOfflineWasm"));
    assert!(!updated.contains("globalThis.const"));
    assert!(!updated.contains("new URL(\"module_bg.wasm\",importMeta.url)"));
  }
}
