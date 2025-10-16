use std::collections::BTreeSet;

use crate::project::OfflineProjectLayout;

/// Generate candidate paths for resolving a markdown asset reference.
///
/// References can appear relative to the module, the optional asset slug, or via explicit
/// leading/trailing slashes. The generator expands the provided value into a deterministic
/// set of possibilities that can be matched against the collected asset map.
pub fn generate_asset_candidates(
    layout: &OfflineProjectLayout,
    module_id: &str,
    asset_slug: Option<&str>,
    path: &str,
) -> Vec<String> {
    if path.is_empty() {
        return Vec::new();
    }

    let mut builder = CandidateBuilder::new(layout, module_id, asset_slug, path);

    builder.add_trimmed_candidate();
    builder.add_slug_candidates();
    builder.add_module_scope_candidates();

    builder.finish()
}

struct CandidateBuilder<'a> {
    layout: &'a OfflineProjectLayout<'a>,
    original: &'a str,
    trimmed: Option<&'a str>,
    module: Option<&'a str>,
    slug: Option<&'a str>,
    slug_was_provided: bool,
    seen: BTreeSet<String>,
    result: Vec<String>,
}

impl<'a> CandidateBuilder<'a> {
    fn new(
        layout: &'a OfflineProjectLayout<'a>,
        module_id: &'a str,
        asset_slug: Option<&'a str>,
        path: &'a str,
    ) -> Self {
        let trimmed_value = path.trim_matches('/');
        let trimmed = if trimmed_value.is_empty() {
            None
        } else {
            Some(trimmed_value)
        };

        let module_value = module_id.trim_matches('/');
        let module = if module_value.is_empty() {
            None
        } else {
            Some(module_value)
        };

        let (slug, slug_was_provided) = match asset_slug {
            Some(value) => {
                let cleaned = value.trim_matches('/');
                if cleaned.is_empty() {
                    (None, true)
                } else {
                    (Some(cleaned), true)
                }
            }
            None => (None, false),
        };

        Self {
            layout,
            original: path,
            trimmed,
            module,
            slug,
            slug_was_provided,
            seen: BTreeSet::new(),
            result: Vec::new(),
        }
    }

    fn add_trimmed_candidate(&mut self) {
        if let Some(path) = self.trimmed {
            self.push(path.to_string());
        }
    }

    fn add_slug_candidates(&mut self) {
        let Some(path) = self.trimmed else {
            return;
        };

        if let Some(slug) = self.slug {
            self.push(format!("{slug}/{path}"));
            if let Some(module) = self.module {
                self.push(format!("{module}/{slug}/{path}"));
            }
        } else if self.slug_was_provided {
            if let Some(module) = self.module {
                self.push(format!("{module}/{path}"));
            }
        }
    }

    fn add_module_scope_candidates(&mut self) {
        let (Some(module), Some(path)) = (self.module, self.trimmed) else {
            return;
        };

        self.push(format!(
            "{module}/{}/{}",
            self.layout.module_assets_dir, path
        ));
        self.push(format!("{module}/{path}"));
    }

    fn finish(mut self) -> Vec<String> {
        self.push(self.original.to_string());
        self.result
    }

    fn push(&mut self, candidate: String) {
        if self.seen.insert(candidate.clone()) {
            self.result.push(candidate);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::generate_asset_candidates;
    use crate::project::OfflineProjectLayout;

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
    fn returns_empty_for_blank_paths() {
        let layout = layout();
        assert!(generate_asset_candidates(&layout, "module", None, "").is_empty());
    }

    #[test]
    fn keeps_original_path_when_no_normalisation_possible() {
        let layout = layout();
        let candidates = generate_asset_candidates(&layout, "module", None, "/");
        assert_eq!(candidates, vec!["/".to_string()]);
    }

    #[test]
    fn generates_candidates_for_module_and_slug_scopes() {
        let layout = layout();
        let candidates =
            generate_asset_candidates(&layout, "safety", Some("week-1"), "images/photo.png");

        assert_eq!(candidates, vec![
            "images/photo.png".to_string(),
            "week-1/images/photo.png".to_string(),
            "safety/week-1/images/photo.png".to_string(),
            format!("safety/{}/images/photo.png", layout.module_assets_dir),
            "safety/images/photo.png".to_string(),
        ]);
    }

    #[test]
    fn deduplicates_candidates_for_empty_slug() {
        let layout = layout();
        let candidates = generate_asset_candidates(&layout, "safety", Some("/"), "docs/intro.md");
        assert_eq!(candidates, vec![
            "docs/intro.md".to_string(),
            "safety/docs/intro.md".to_string(),
            format!("safety/{}/docs/intro.md", layout.module_assets_dir),
        ]);
    }
}
