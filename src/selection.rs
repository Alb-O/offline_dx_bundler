//! Traits used to filter which collections are included in the offline bundle.

/// Trait describing selection filters for offline build content.
pub trait CollectionInclusion {
    /// Returns `true` when the collection should be included in the offline bundle.
    fn is_included(&self, collection_id: &str) -> bool;
}
