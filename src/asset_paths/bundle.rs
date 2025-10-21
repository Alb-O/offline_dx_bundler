use crate::project::OfflineProjectLayout;

/// Produce the canonical on-disk path for an asset in the offline bundle.
///
/// The generated path always uses forward slashes so that the resulting manifest works on
/// every platform, regardless of the native directory separator that was used when the
/// files were discovered on disk.
pub fn make_offline_asset_path(
    layout: &OfflineProjectLayout,
    collection_id: &str,
    relative_path: &str,
) -> String {
    format!(
        "{}/{}/{}",
        layout.collections_dir_name, collection_id, relative_path
    )
    .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::make_offline_asset_path;
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
    fn joins_collection_and_relative_paths() {
        let layout = layout();
        let result = make_offline_asset_path(&layout, "deckhand", "images/logo.png");
        assert_eq!(result, "programs/deckhand/images/logo.png");
    }

    #[test]
    fn normalises_backslashes_from_windows_inputs() {
        let layout = layout();
        let result = make_offline_asset_path(&layout, "bridge", "videos\\\\intro.mp4");
        assert_eq!(result, "programs/bridge/videos/intro.mp4");
    }
}
