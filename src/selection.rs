//! Traits used to filter which programs are included in the offline bundle.

/// Trait describing selection filters for offline build content.
pub trait ProgramInclusion {
  /// Returns `true` when the program should be included in the offline bundle.
  fn is_included(&self, program_id: &str) -> bool;
}
