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

	/// Indicates if any existing symlink at the [`Symlink::target_path`] should
	/// be replaced by this item.
	///
	/// # NOTE
	/// It will only replace existing symlink.
	#[serde(default = "default_replace_value")]
	pub replace: bool,
}

/// Provides the default value for [`Symlink::replace`].
const fn default_replace_value() -> bool {
	true
}
