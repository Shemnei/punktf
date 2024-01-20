use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{merge::MergeMode, profile::Shared};

#[derive(Debug, Serialize, Deserialize)]
pub struct Item {
	#[serde(flatten)]
	pub shared: Shared,

	pub path: PathBuf,

	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub rename: Option<PathBuf>,

	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub overwrite_target: Option<PathBuf>,

	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub merge: Option<MergeMode>,
}
