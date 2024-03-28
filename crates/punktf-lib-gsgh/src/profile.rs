use crate::{
	env::Environment, hook::Hook, item::Item, prio::Priority, transform::Transformer,
	version::Version,
};
use std::{path::PathBuf, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
	#[error("invalid profile: {0}")]
	InvalidProfile(#[from] serde_yaml::Error),
	#[error("unsupported version: {0}")]
	UnsupportedVersion(Version),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Wrapper struct to be able to first parse only the version and then choose
/// the appropriate profile struct for it to do version compatible parsing.
#[repr(transparent)]
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct ProfileVersion {
	pub version: Version,
}

impl Default for ProfileVersion {
	fn default() -> Self {
		Self {
			version: Version::ZERO,
		}
	}
}

impl From<ProfileVersion> for Version {
	fn from(value: ProfileVersion) -> Self {
		value.version
	}
}

impl AsRef<Version> for ProfileVersion {
	fn as_ref(&self) -> &Version {
		&self.version
	}
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shared {
	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub priority: Option<Priority>,

	#[serde(rename = "env", skip_serializing_if = "Environment::is_empty", default)]
	pub environment: Environment,

	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pub transformers: Vec<Transformer>,

	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub pre_hook: Option<Hook>,

	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub post_hook: Option<Hook>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Profile {
	#[serde(flatten)]
	pub version: ProfileVersion,

	#[serde(flatten)]
	pub shared: Shared,

	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pub aliases: Vec<String>,

	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pub extends: Vec<String>,

	#[serde(skip_serializing_if = "Option::is_none", default)]
	pub target: Option<PathBuf>,

	#[serde(skip_serializing_if = "Vec::is_empty", default)]
	pub items: Vec<Item>,
}

impl Profile {
	pub const VERSION: Version = Version::new(1, 0, 0);
}

impl FromStr for Profile {
	type Err = Error;

	fn from_str(s: &str) -> Result<Self> {
		let version: Version = serde_yaml::from_str::<ProfileVersion>(s)?.version;

		// No version or explicit zero version
		if version == Version::ZERO {
			return Err(Error::UnsupportedVersion(version));
		}

		// Version matching
		if Self::VERSION.compatible(version) {
			serde_yaml::from_str(s).map_err(Into::into)
		} else {
			Err(Error::UnsupportedVersion(version))
		}
	}
}
