//! Helpers for resolving and normalising asset paths for offline bundles.
//!
//! This module intentionally splits the responsibilities into focused submodules so that
//! the logic for filtering references, building bundle-relative paths, and expanding
//! candidate references can be tested independently. The same code is shared between the
//! build-time manifest generator and the runtime helpers.

mod bundle;
mod candidates;
mod filters;

pub use bundle::make_offline_asset_path;
pub use candidates::generate_asset_candidates;
pub use filters::should_ignore_asset_reference;
