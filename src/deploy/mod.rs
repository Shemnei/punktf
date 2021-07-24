use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;
use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::Item;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemStatus {
	Success,
	Failed(Cow<'static, str>),
	Skipped(Cow<'static, str>),
}

impl ItemStatus {
	pub fn success() -> Self {
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

	pub fn is_failed(&self) -> bool {
		matches!(self, &Self::Failed(_))
	}

	pub fn is_skipped(&self) -> bool {
		matches!(self, &Self::Skipped(_))
	}
}

impl fmt::Display for ItemStatus {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Success => f.write_str("Success"),
			Self::Failed(reason) => write!(f, "Failed: {}", reason),
			Self::Skipped(reason) => write!(f, "Skipped: {}", reason),
		}
	}
}

impl<E> From<E> for ItemStatus
where
	E: std::error::Error,
{
	fn from(value: E) -> Self {
		Self::failed(value.to_string())
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployedItem {
	status: ItemStatus,
	item: Item,
}

impl AsRef<Item> for DeployedItem {
	fn as_ref(&self) -> &Item {
		&self.item
	}
}

impl Deref for DeployedItem {
	type Target = Item;

	fn deref(&self) -> &Self::Target {
		&self.item
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeploymentStatus {
	Success,
	Failed(Cow<'static, str>),
}

impl DeploymentStatus {
	pub fn success() -> Self {
		Self::Success
	}

	pub fn failed<S: Into<Cow<'static, str>>>(reason: S) -> Self {
		Self::Failed(reason.into())
	}

	pub fn is_success(&self) -> bool {
		self == &Self::Success
	}

	pub fn is_failed(&self) -> bool {
		matches!(self, &Self::Failed(_))
	}
}

impl fmt::Display for DeploymentStatus {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Success => f.write_str("Success"),
			Self::Failed(reason) => write!(f, "Failed: {}", reason),
		}
	}
}

impl<E> From<E> for DeploymentStatus
where
	E: std::error::Error,
{
	fn from(value: E) -> Self {
		Self::failed(value.to_string())
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployment {
	time_start: DateTime<Utc>,
	time_end: DateTime<Utc>,
	status: DeploymentStatus,
	items: HashMap<PathBuf, DeployedItem>,
}

impl Deployment {
	pub fn time_start(&self) -> &DateTime<Utc> {
		&self.time_start
	}

	pub fn time_end(&self) -> &DateTime<Utc> {
		&self.time_end
	}

	pub fn duration(&self) -> Duration {
		self.time_end - self.time_start
	}

	pub fn status(&self) -> &DeploymentStatus {
		&self.status
	}

	pub fn build() -> DeploymentBuilder {
		DeploymentBuilder::default()
	}
}

#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentBuilder {
	time_start: DateTime<Utc>,
	items: HashMap<PathBuf, DeployedItem>,
}

impl DeploymentBuilder {
	pub fn add_item(&mut self, path: PathBuf, item: Item, status: ItemStatus) -> &mut Self {
		self.items.insert(path, DeployedItem { item, status });
		self
	}

	pub fn success(self) -> Deployment {
		Deployment {
			time_start: self.time_start,
			time_end: Utc::now(),
			status: DeploymentStatus::Success,
			items: self.items,
		}
	}

	pub fn failed<S: Into<Cow<'static, str>>>(self, reason: S) -> Deployment {
		Deployment {
			time_start: self.time_start,
			time_end: Utc::now(),
			status: DeploymentStatus::Failed(reason.into()),
			items: self.items,
		}
	}
}

impl Default for DeploymentBuilder {
	fn default() -> Self {
		Self {
			time_start: Utc::now(),
			items: HashMap::new(),
			// TODO: INVESTIGATE - Causes stack overflow???
			//..Default::default()
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn deployment_builder() {
		let builder = Deployment::build();
		let deployment = builder.success();

		assert!(deployment.status().is_success());
		assert!(deployment.duration() >= Duration::seconds(0));
	}
}
