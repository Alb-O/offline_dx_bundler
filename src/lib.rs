#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![allow(clippy::module_inception)]

pub mod asset_paths;
#[cfg(not(target_arch = "wasm32"))]
pub mod builder;
#[cfg(not(target_arch = "wasm32"))]
pub mod manifest;
pub mod models;
pub mod project;
pub mod selection;
#[cfg(not(target_arch = "wasm32"))]
pub mod bundle;

#[cfg(not(target_arch = "wasm32"))]
pub use builder::{BuildResult, OfflineArtifacts, OfflineBuilder};
pub use project::{OfflineBuildContext, OfflineProjectLayout};
pub use selection::ProgramInclusion;
