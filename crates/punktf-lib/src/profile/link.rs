//! Defines definitions for a [`Symlink`].

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A symlink to be created during the deployment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symlink {
	/// Absolute path of the link source.
	pub source_path: PathBuf,

	/// Absolute path of the link target.
	pub target_path: PathBuf,
}
