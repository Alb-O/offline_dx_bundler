use std::collections::BTreeSet;

use crate::project::OfflineProjectLayout;

/// Generate candidate paths for resolving a markdown asset reference.
///
/// References can appear relative to the entry, the optional asset slug, or via explicit
/// leading/trailing slashes. The generator expands the provided value into a deterministic
/// set of possibilities that can be matched against the collected asset map.
pub fn generate_asset_candidates(
    layout: &OfflineProjectLayout,
    entry_id: &str,
    asset_slug: Option<&str>,
    path: &str,
) -> Vec<String> {
    if path.is_empty() {
        return Vec::new();
    }

    let mut builder = CandidateBuilder::new(layout, entry_id, asset_slug, path);

    builder.add_trimmed_candidate();
    builder.add_slug_candidates();
    builder.add_entry_scope_candidates();

    builder.finish()
}

struct CandidateBuilder<'a> {
    layout: &'a OfflineProjectLayout,
    original: &'a str,
    trimmed: Option<&'a str>,
    entry: Option<&'a str>,
    slug: Option<&'a str>,
    slug_was_provided: bool,
    seen: BTreeSet<String>,
    result: Vec<String>,
}

impl<'a> CandidateBuilder<'a> {
    fn new(
        layout: &'a OfflineProjectLayout,
        entry_id: &'a str,
        asset_slug: Option<&'a str>,
        path: &'a str,
    ) -> Self {
        let trimmed_value = path.trim_matches('/');
        let trimmed = if trimmed_value.is_empty() {
            None
        } else {
            Some(trimmed_value)
        };

        let entry_value = entry_id.trim_matches('/');
        let entry = if entry_value.is_empty() {
            None
        } else {
            Some(entry_value)
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
            entry,
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
            if let Some(entry) = self.entry {
                self.push(format!("{entry}/{slug}/{path}"));
            }
        } else if self.slug_was_provided {
            if let Some(entry) = self.entry {
                self.push(format!("{entry}/{path}"));
            }
        }
    }

    fn add_entry_scope_candidates(&mut self) {
        let (Some(entry), Some(path)) = (self.entry, self.trimmed) else {
            return;
        };

        self.push(format!(
            "{entry}/{}/{}",
            self.layout.entry_assets_dir(), path
        ));
        self.push(format!("{entry}/{path}"));
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
    fn returns_empty_for_blank_paths() {
        let layout = layout();
        assert!(generate_asset_candidates(&layout, "entry", None, "").is_empty());
    }

    #[test]
    fn keeps_original_path_when_no_normalisation_possible() {
        let layout = layout();
        let candidates = generate_asset_candidates(&layout, "entry", None, "/");
        assert_eq!(candidates, vec!["/".to_string()]);
    }

    #[test]
    fn generates_candidates_for_entry_and_slug_scopes() {
        let layout = layout();
        let candidates =
            generate_asset_candidates(&layout, "safety", Some("week-1"), "images/photo.png");

        assert_eq!(candidates, vec![
            "images/photo.png".to_string(),
            "week-1/images/photo.png".to_string(),
            "safety/week-1/images/photo.png".to_string(),
            format!("safety/{}/images/photo.png", layout.entry_assets_dir()),
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
            format!("safety/{}/docs/intro.md", layout.entry_assets_dir()),
        ]);
    }
}
