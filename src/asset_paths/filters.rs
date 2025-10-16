use regex::Regex;

fn asset_reference_ignores() -> &'static [Regex] {
    use std::sync::OnceLock;

    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS
        .get_or_init(|| {
            vec![
                Regex::new(r"(?i)^https?://").expect("invalid http(s) regex"),
                Regex::new(r"(?i)^data:").expect("invalid data URI regex"),
                Regex::new(r"(?i)^mailto:").expect("invalid mailto regex"),
            ]
        })
        .as_slice()
}

/// Determine whether a markdown asset reference should be ignored during offline analysis.
///
/// External URLs and data URIs are intentionally excluded, since they cannot be embedded into the
/// offline bundle and require a network connection to resolve anyway.
pub fn should_ignore_asset_reference(value: &str) -> bool {
    asset_reference_ignores()
        .iter()
        .any(|pattern| pattern.is_match(value))
}

#[cfg(test)]
mod tests {
    use super::should_ignore_asset_reference;

    #[test]
    fn ignores_http_urls() {
        assert!(should_ignore_asset_reference("https://example.com"));
        assert!(should_ignore_asset_reference("HTTP://example.com"));
    }

    #[test]
    fn ignores_data_uris() {
        assert!(should_ignore_asset_reference("data:image/png;base64,abc"));
    }

    #[test]
    fn ignores_mailto_links() {
        assert!(should_ignore_asset_reference("mailto:user@example.com"));
    }

    #[test]
    fn keeps_relative_paths() {
        assert!(!should_ignore_asset_reference("images/photo.png"));
    }
}
