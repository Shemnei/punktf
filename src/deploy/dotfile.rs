use std::borrow::Cow;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Dotfile;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DotfileStatus {
	Success,
	Failed(Cow<'static, str>),
	Skipped(Cow<'static, str>),
}

impl DotfileStatus {
	pub const fn success() -> Self {
		Self::Success
	}

	pub fn failed<S: Into<Cow<'static, str>>>(reason: S) -> Self {
		Self::Failed(reason.into())
	}

	pub fn skipped<S: Into<Cow<'static, str>>>(reason: S) -> Self {
		Self::Skipped(reason.into())
	}

	pub fn is_success(&self) -> bool {
		self == &Self::Success
	}

	pub const fn is_failed(&self) -> bool {
		matches!(self, &Self::Failed(_))
	}

	pub const fn is_skipped(&self) -> bool {
		matches!(self, &Self::Skipped(_))
	}
}

impl fmt::Display for DotfileStatus {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Success => f.write_str("Success"),
			Self::Failed(reason) => write!(f, "Failed: {}", reason),
			Self::Skipped(reason) => write!(f, "Skipped: {}", reason),
		}
	}
}

impl<E> From<E> for DotfileStatus
where
	E: std::error::Error,
{
	fn from(value: E) -> Self {
		Self::failed(value.to_string())
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeployedDotfileKind {
	Dotfile(Dotfile),
	// PathBuf is `parent` dotfile path. The parent should always be of type `Dotfile(_)`.
	Child(PathBuf),
}

impl DeployedDotfileKind {
	pub const fn is_dotfile(&self) -> bool {
		matches!(self, Self::Dotfile(_))
	}

	pub const fn is_child(&self) -> bool {
		matches!(self, Self::Child(_))
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployedDotfile {
	pub status: DotfileStatus,
	pub kind: DeployedDotfileKind,
}

impl DeployedDotfile {
	pub const fn status(&self) -> &DotfileStatus {
		&self.status
	}

	pub const fn kind(&self) -> &DeployedDotfileKind {
		&self.kind
	}
}
