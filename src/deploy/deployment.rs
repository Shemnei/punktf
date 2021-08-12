use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, SystemTimeError};

use serde::{Deserialize, Serialize};

use super::item::{DeployedItem, DeployedItemKind, ItemStatus};
use crate::{Item, Priority};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeploymentStatus {
	Success,
	Failed(Cow<'static, str>),
}

impl DeploymentStatus {
	pub const fn success() -> Self {
		Self::Success
	}

	pub fn failed<S: Into<Cow<'static, str>>>(reason: S) -> Self {
		Self::Failed(reason.into())
	}

	pub fn is_success(&self) -> bool {
		self == &Self::Success
	}

	pub const fn is_failed(&self) -> bool {
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
	time_start: SystemTime,
	time_end: SystemTime,
	status: DeploymentStatus,
	items: HashMap<PathBuf, DeployedItem>,
}

impl Deployment {
	pub const fn time_start(&self) -> &SystemTime {
		&self.time_start
	}

	pub const fn time_end(&self) -> &SystemTime {
		&self.time_end
	}

	pub fn duration(&self) -> Result<Duration, SystemTimeError> {
		self.time_end.duration_since(self.time_start)
	}

	pub const fn status(&self) -> &DeploymentStatus {
		&self.status
	}

	pub const fn items(&self) -> &HashMap<PathBuf, DeployedItem> {
		&self.items
	}

	pub fn build() -> DeploymentBuilder {
		DeploymentBuilder::default()
	}
}

#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentBuilder {
	time_start: SystemTime,
	items: HashMap<PathBuf, DeployedItem>,
}

impl DeploymentBuilder {
	pub fn add_item(&mut self, path: PathBuf, item: Item, status: ItemStatus) -> &mut Self {
		self.items.insert(
			path,
			DeployedItem {
				kind: DeployedItemKind::Item(item),
				status,
			},
		);
		self
	}

	pub fn add_child(&mut self, path: PathBuf, parent: PathBuf, status: ItemStatus) -> &mut Self {
		self.items.insert(
			path,
			DeployedItem {
				kind: DeployedItemKind::Child(parent),
				status,
			},
		);
		self
	}

	pub fn contains<P: AsRef<Path>>(&self, path: P) -> bool {
		self.items.contains_key(path.as_ref())
	}

	pub fn get_item<P: AsRef<Path>>(&self, path: P) -> Option<&Item> {
		let mut value = self.items.get(path.as_ref())?;

		loop {
			match &value.kind {
				DeployedItemKind::Item(item) => return Some(item),
				DeployedItemKind::Child(parent_path) => value = self.items.get(parent_path)?,
			}
		}
	}

	/// Only gets the item if all items in the chain are deployed
	pub fn get_deployed_item<P: AsRef<Path>>(&self, path: P) -> Option<&Item> {
		let mut value = self.items.get(path.as_ref())?;

		loop {
			if !value.status.is_success() {
				return None;
			}

			match &value.kind {
				DeployedItemKind::Item(item) => return Some(item),
				DeployedItemKind::Child(parent_path) => value = self.items.get(parent_path)?,
			}
		}
	}

	pub fn get_priority<P: AsRef<Path>>(&self, path: P) -> Option<Option<Priority>> {
		self.get_deployed_item(path).map(|item| item.priority)
	}

	pub fn is_deployed<P: AsRef<Path>>(&self, path: P) -> Option<bool> {
		self.items
			.get(path.as_ref())
			.map(|item| item.status.is_success())
	}

	pub fn success(self) -> Deployment {
		Deployment {
			time_start: self.time_start,
			time_end: SystemTime::now(),
			status: DeploymentStatus::Success,
			items: self.items,
		}
	}

	pub fn failed<S: Into<Cow<'static, str>>>(self, reason: S) -> Deployment {
		Deployment {
			time_start: self.time_start,
			time_end: SystemTime::now(),
			status: DeploymentStatus::Failed(reason.into()),
			items: self.items,
		}
	}
}

impl Default for DeploymentBuilder {
	fn default() -> Self {
		Self {
			time_start: SystemTime::now(),
			items: HashMap::new(),
		}
	}
}

#[cfg(test)]
mod tests {
	use color_eyre::Result;

	use super::*;

	#[test]
	fn deployment_builder() -> Result<()> {
		let builder = Deployment::build();
		let deployment = builder.success();

		assert!(deployment.status().is_success());
		assert!(deployment.duration()? >= Duration::from_secs(0));

		Ok(())
	}
}
