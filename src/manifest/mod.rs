//! Offline manifest generation broken into focused submodules for easier testing.

mod generation;
mod markdown;
mod scanning;

pub use generation::generate_offline_manifest;
#[allow(unused_imports)]
pub use markdown::{
    collect_markdown_asset_references, parse_entry_markdown, parse_order_from_id,
    resolve_markdown_assets,
};
#[allow(unused_imports)]
pub use scanning::{collect_assets_recursively, sanitize_const_name};
