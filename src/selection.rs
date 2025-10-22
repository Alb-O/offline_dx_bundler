//! Helpers used to filter which collections are included in the offline bundle.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Trait describing selection filters for offline build content.
pub trait CollectionInclusion {
  /// Returns `true` when the collection should be included in the offline bundle.
  fn is_included(&self, collection_id: &str) -> bool;
}

/// Default selection file name searched for in collection directories.
pub const DEFAULT_SELECTION_FILE: &str = "collections.local.json";

/// Configuration file layout for selecting which collections to compile.
#[derive(Debug, Default, Deserialize)]
struct CollectionSelectionFile {
  #[serde(default)]
  include: Vec<String>,
  #[serde(default)]
  exclude: Vec<String>,
}

/// Selection helper allowing build-time filtering of authored collections.
#[derive(Debug, Clone, Default)]
pub struct CollectionSelection {
  include: Option<BTreeSet<String>>,
  exclude: BTreeSet<String>,
}

/// Errors that can occur while loading the selection configuration.
#[derive(Debug)]
pub enum CollectionSelectionError {
  /// Failed to read the selection file from disk.
  Io {
    /// Path that caused the error.
    path: PathBuf,
    /// Source I/O error.
    source: std::io::Error,
  },
  /// Failed to parse the JSON selection file.
  Parse {
    /// Path that caused the error.
    path: PathBuf,
    /// Source parse error.
    source: serde_json::Error,
  },
}

impl CollectionSelection {
  /// Load configuration from the selection file if present.
  pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, CollectionSelectionError> {
    let path = path.as_ref();
    let contents = match fs::read_to_string(path) {
      Ok(contents) => contents,
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
        return Ok(Self::default());
      }
      Err(err) => {
        return Err(CollectionSelectionError::Io {
          path: path.to_path_buf(),
          source: err,
        });
      }
    };

    let file: CollectionSelectionFile =
      serde_json::from_str(&contents).map_err(|err| CollectionSelectionError::Parse {
        path: path.to_path_buf(),
        source: err,
      })?;
    Ok(Self::from(file))
  }

  /// Determine whether a collection should be compiled into the bundle.
  pub fn is_included(&self, collection_id: &str) -> bool {
    if self
      .exclude
      .iter()
      .any(|value| scope_matches(value, collection_id))
    {
      return false;
    }

    match &self.include {
      Some(include) => include
        .iter()
        .any(|value| scope_matches(value, collection_id)),
      None => true,
    }
  }

  /// Returns true when no filtering rules are active.
  #[cfg(test)]
  fn is_unfiltered(&self) -> bool {
    self.include.as_ref().is_none() && self.exclude.is_empty()
  }
}

impl CollectionInclusion for CollectionSelection {
  fn is_included(&self, collection_id: &str) -> bool {
    CollectionSelection::is_included(self, collection_id)
  }
}

impl From<CollectionSelectionFile> for CollectionSelection {
  fn from(file: CollectionSelectionFile) -> Self {
    let include = normalise_list(file.include);
    let exclude = normalise_list(file.exclude);

    Self {
      include: (!include.is_empty()).then_some(include),
      exclude,
    }
  }
}

impl std::fmt::Display for CollectionSelectionError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Io { path, source } => {
        write!(f, "failed to read {}: {}", path.display(), source)
      }
      Self::Parse { path, source } => {
        write!(f, "failed to parse {}: {}", path.display(), source)
      }
    }
  }
}

impl std::error::Error for CollectionSelectionError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    match self {
      Self::Io { source, .. } => Some(source),
      Self::Parse { source, .. } => Some(source),
    }
  }
}

/// Convert a list of raw identifiers into a sorted, de-duplicated set.
///
/// Values are trimmed and empty entries are discarded to simplify downstream filtering logic.
fn normalise_list(values: impl IntoIterator<Item = String>) -> BTreeSet<String> {
  values
    .into_iter()
    .map(|value| value.trim().trim_matches('/').to_string())
    .filter(|value| !value.is_empty())
    .collect()
}

fn scope_matches(rule: &str, candidate: &str) -> bool {
  if candidate == rule {
    return true;
  }

  candidate
    .strip_prefix(rule)
    .is_some_and(|suffix| suffix.starts_with('/'))
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::tempdir;

  #[test]
  fn defaults_to_including_all_collections() {
    let selection = CollectionSelection::default();
    assert!(selection.is_included("any"));
    assert!(selection.is_unfiltered());
  }

  #[test]
  fn excludes_collections_listed_in_config() {
    let selection = CollectionSelection::from(CollectionSelectionFile {
      include: Vec::new(),
      exclude: vec!["P001".into(), String::new(), " P002 ".into()],
    });

    assert!(!selection.is_included("P001"));
    assert!(!selection.is_included("P002"));
    assert!(selection.is_included("P003"));
  }

  #[test]
  fn excludes_nested_collections_with_parent_scope() {
    let selection = CollectionSelection::from(CollectionSelectionFile {
      include: Vec::new(),
      exclude: vec!["P001".into()],
    });

    assert!(!selection.is_included("P001"));
    assert!(!selection.is_included("P001/module-a"));
  }

  #[test]
  fn includes_nested_collections_with_parent_scope() {
    let selection = CollectionSelection::from(CollectionSelectionFile {
      include: vec!["P001".into()],
      exclude: Vec::new(),
    });

    assert!(selection.is_included("P001"));
    assert!(selection.is_included("P001/module-a"));
    assert!(!selection.is_included("P002"));
  }

  #[test]
  fn allows_overriding_child_exclusions() {
    let selection = CollectionSelection::from(CollectionSelectionFile {
      include: vec!["P001/module-a".into()],
      exclude: vec!["P001/module-a/draft".into()],
    });

    assert!(!selection.is_included("P001"));
    assert!(selection.is_included("P001/module-a"));
    assert!(!selection.is_included("P001/module-a/draft"));
  }

  #[test]
  fn honours_include_overrides() {
    let selection = CollectionSelection::from(CollectionSelectionFile {
      include: vec!["A".into(), "B".into()],
      exclude: vec!["B".into(), "C".into()],
    });

    assert!(selection.is_included("A"));
    assert!(!selection.is_included("B"));
    assert!(!selection.is_included("C"));
    assert!(!selection.is_included("D"));
  }

  #[test]
  fn normalises_whitespace_and_duplicates() {
    let normalised: Vec<String> = normalise_list(vec![
      "  A  ".into(),
      "b".into(),
      "A".into(),
      String::new(),
      "B".into(),
    ])
    .into_iter()
    .collect();

    assert_eq!(normalised, vec![
      String::from("A"),
      String::from("B"),
      String::from("b")
    ]);
  }

  #[test]
  fn load_from_path_returns_default_for_missing_file() {
    let temp = tempdir().expect("failed to create temp dir");
    let path = temp.path().join("collections.local.json");

    let selection = CollectionSelection::load_from_path(&path)
      .expect("missing files should not produce an error");

    assert!(selection.is_unfiltered());
  }

  #[test]
  fn load_from_path_reads_configuration() {
    let temp = tempdir().expect("failed to create temp dir");
    let path = temp.path().join("collections.local.json");
    std::fs::write(
      &path,
      r#"{"include": ["A", "B"], "exclude": ["B", "C", ""]}"#,
    )
    .expect("failed to write selection file");

    let selection =
      CollectionSelection::load_from_path(&path).expect("configuration should load successfully");

    assert!(!selection.is_unfiltered());
    assert!(selection.is_included("A"));
    assert!(!selection.is_included("B"));
    assert!(!selection.is_included("C"));
    assert!(!selection.is_included("missing"));
  }
}
