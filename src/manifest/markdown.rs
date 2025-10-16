//! Markdown parsing helpers used during manifest generation.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use gray_matter::{Matter, engine::YAML};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::asset_paths::{
    generate_asset_candidates, make_offline_asset_path, should_ignore_asset_reference,
};
use crate::models::{AssetEntry, ModuleFrontmatterRecord};
use crate::project::OfflineProjectLayout;

/// Parse the numeric ordering prefix from a module identifier if present.
pub fn parse_order_from_id(id: &str) -> Option<usize> {
    let prefix = id.split_once('-').map(|(value, _)| value).unwrap_or(id);
    let digits: String = prefix.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        None
    } else {
        digits.parse::<usize>().ok()
    }
}

/// Collect asset references (links, images and inline HTML) from markdown content.
pub fn collect_markdown_asset_references(markdown: &str) -> BTreeSet<String> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);

    let parser = Parser::new_ext(markdown, options);
    let mut references = BTreeSet::new();

    for event in parser {
        match event {
            Event::Start(Tag::Image { .. }) | Event::End(TagEnd::Image) => {}
            Event::Start(Tag::Link { dest_url, .. }) => {
                add_reference(&mut references, &dest_url);
            }
            Event::End(TagEnd::Link) => {}
            Event::Html(html) | Event::InlineHtml(html) => {
                extract_inline_asset_values(&html, &mut references);
            }
            Event::Text(text) => {
                if text.starts_with("![") || text.contains("](") {
                    extract_inline_asset_values(&text, &mut references);
                }
            }
            _ => {}
        }
    }

    references
}

/// Resolve asset references for a specific module against the discovered asset map.
pub fn resolve_markdown_assets(
    layout: &OfflineProjectLayout,
    references: &BTreeSet<String>,
    asset_map: &BTreeMap<(String, String), AssetEntry>,
    program_id: &str,
    module_id: &str,
    asset_slug: Option<&str>,
) -> (Vec<String>, Vec<String>) {
    let mut resolved = BTreeSet::new();
    let mut unresolved = Vec::new();

    for reference in references {
        let candidates = generate_asset_candidates(layout, module_id, asset_slug, reference);
        let mut found = false;

        for candidate in candidates {
            if let Some(entry) = asset_map.get(&(program_id.to_string(), candidate)) {
                resolved.insert(make_offline_asset_path(
                    layout,
                    &entry.program_id,
                    &entry.relative_path,
                ));
                found = true;
                break;
            }
        }

        if !found {
            unresolved.push(reference.clone());
        }
    }

    (resolved.into_iter().collect(), unresolved)
}

/// Parse a module markdown file, extracting frontmatter metadata and the content body.
pub fn parse_module_markdown(
    module_markdown_path: &Path,
) -> Option<(ModuleFrontmatterRecord, String)> {
    let content = fs::read_to_string(module_markdown_path).ok()?;
    let matter = Matter::<YAML>::new();
    let parsed = matter.parse(&content).ok()?;

    let frontmatter: ModuleFrontmatterRecord = parsed
        .data
        .and_then(|yaml| serde_yaml::from_value::<ModuleFrontmatterRecord>(yaml).ok())
        .unwrap_or_default();

    Some((frontmatter, parsed.content))
}

pub(super) fn extract_first_heading(body: &str) -> Option<String> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
    options.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);

    let parser = Parser::new_ext(body, options);
    let mut in_heading = false;
    let mut heading_text = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                in_heading = true;
                heading_text.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                if in_heading && !heading_text.trim().is_empty() {
                    return Some(heading_text.trim().to_string());
                }
                in_heading = false;
            }
            Event::Text(text) if in_heading => {
                heading_text.push_str(&text);
            }
            _ => {}
        }
    }

    None
}

fn add_reference(references: &mut BTreeSet<String>, value: &str) {
    if should_ignore_asset_reference(value) {
        return;
    }
    references.insert(value.to_string());
}

fn extract_inline_asset_values(fragment: &str, references: &mut BTreeSet<String>) {
    extract_attribute_values(fragment, "src", references);
    extract_attribute_values(fragment, "href", references);
    extract_attribute_values(fragment, "poster", references);

    let mut chars = fragment.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '!' && chars.peek() == Some(&'[') {
            while let Some(ch) = chars.next() {
                if ch == ']' && chars.peek() == Some(&'(') {
                    chars.next();
                    let mut path = String::new();
                    for ch in chars.by_ref() {
                        if ch == ')' {
                            break;
                        }
                        path.push(ch);
                    }
                    add_reference(references, path.trim());
                    break;
                }
            }
        }
    }
}

fn extract_attribute_values(fragment: &str, attribute: &str, references: &mut BTreeSet<String>) {
    let pattern = format!("{}=\"", attribute);
    let mut start = 0;

    while let Some(pos) = fragment[start..].find(&pattern) {
        let attr_start = start + pos + pattern.len();
        if let Some(end) = fragment[attr_start..].find('"') {
            let value = &fragment[attr_start..attr_start + end];
            add_reference(references, value);
            start = attr_start + end + 1;
        } else {
            break;
        }
    }

    let pattern_single = format!("{}='", attribute);
    start = 0;
    while let Some(pos) = fragment[start..].find(&pattern_single) {
        let attr_start = start + pos + pattern_single.len();
        if let Some(end) = fragment[attr_start..].find('\'') {
            let value = &fragment[attr_start..attr_start + end];
            add_reference(references, value);
            start = attr_start + end + 1;
        } else {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn parses_numeric_prefix_from_id() {
        assert_eq!(parse_order_from_id("001-intro"), Some(1));
        assert_eq!(parse_order_from_id("intro"), None);
    }

    #[test]
    fn collects_asset_references_from_markdown() {
        let markdown = "![Alt](image.png) <img src=\"video.mp4\">";
        let references = collect_markdown_asset_references(markdown);
        assert!(references.contains("image.png"));
        assert!(references.contains("video.mp4"));
    }

    #[test]
    fn resolves_references_against_asset_map() {
        let layout = layout();
        let mut asset_map = BTreeMap::new();
        asset_map.insert(
            ("program".to_string(), "module/assets/image.png".to_string()),
            AssetEntry {
                const_name: "CONST".into(),
                literal_path: "".into(),
                program_id: "program".into(),
                relative_path: "module/assets/image.png".into(),
            },
        );

        let references = BTreeSet::from(["image.png".to_string()]);
        let (resolved, unresolved) =
            resolve_markdown_assets(&layout, &references, &asset_map, "program", "module", None);

        assert_eq!(unresolved.len(), 0);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0], "programs/program/module/assets/image.png");
    }
}
