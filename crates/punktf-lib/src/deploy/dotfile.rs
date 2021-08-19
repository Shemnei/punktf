//! Statistics and information about the final state of a deployed
//! [dotfile](`crate::Dotfile`).

use std::borrow::Cow;
use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Dotfile;

/// Contains the status of the dotfile operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DotfileStatus {
	/// The dotfile was successfully created.
	Success,
	/// The dotfile deployment failed.
	Failed(Cow<'static, str>),
	/// The dotfile deployment was skipped.
	Skipped(Cow<'static, str>),
}

impl DotfileStatus {
	/// Marks the dotfile operation as successful.
	pub const fn success() -> Self {
		Self::Success
	}

	/// Marks the dotfile operation as failed.
	pub fn failed<S: Into<Cow<'static, str>>>(reason: S) -> Self {
		Self::Failed(reason.into())
	}

	/// Indicates that the dotfile opeartion was skipped.
	pub fn skipped<S: Into<Cow<'static, str>>>(reason: S) -> Self {
		Self::Skipped(reason.into())
	}

	/// Checks if the dotfile operation was successful.
	pub fn is_success(&self) -> bool {
		self == &Self::Success
	}

	/// Checks if the dotfile operation has failed.
	pub const fn is_failed(&self) -> bool {
		matches!(self, &Self::Failed(_))
	}

	/// Checks if the dotfile operation was skipped.
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

/// Defines the type of dotfile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeployedDotfileKind {
	/// A normal dotfile.
	Dotfile(Dotfile),
	/// A dotfile that is contained in a directory that is deployed.
	///
	/// PathBuf is the deploy path of the `parent` dotfile.
	/// The parent should always be of type `Dotfile(_)`.
	Child(PathBuf),
}

impl DeployedDotfileKind {
	/// Checks whether the deployed dotfile type is a normal dotfile.
	pub const fn is_dotfile(&self) -> bool {
		matches!(self, Self::Dotfile(_))
	}

	/// Checks whether the deployed dotfile type is a child dotfile.
	pub const fn is_child(&self) -> bool {
		matches!(self, Self::Child(_))
	}
}

/// Stores the result of a dotfile deployment operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployedDotfile {
	/// The status of the deployed dotfile.
	pub status: DotfileStatus,

	/// The kind of the deployed dotfile.
	pub kind: DeployedDotfileKind,
}

impl DeployedDotfile {
	/// Returns the status of the dotfile operation.
	pub const fn status(&self) -> &DotfileStatus {
		&self.status
	}

	/// Retures the kind of the dotfile operation.
	pub const fn kind(&self) -> &DeployedDotfileKind {
		&self.kind
	}
}
