use std::collections::VecDeque;
use std::fs::ReadDir;
use std::path::{Path, PathBuf};

use color_eyre::eyre::Context;
use color_eyre::Result;

use super::deployment::{Deployment, DeploymentBuilder};
use crate::deploy::item::ItemStatus;
use crate::template::Template;
use crate::variables::UserVars;
use crate::{DeployTarget, Item, MergeMode, Priority, Profile};

enum ExecutorItem<'a> {
	File {
		item: Item,
		source_path: PathBuf,
		deploy_path: PathBuf,
	},
	Child {
		parent: &'a Item,
		parent_source_path: &'a Path,
		parent_deploy_path: &'a Path,
		// relative path in source
		path: PathBuf,
		source_path: PathBuf,
		deploy_path: PathBuf,
	},
}

impl<'a> ExecutorItem<'a> {
	fn deploy_path(&self) -> &Path {
		match self {
			Self::File { deploy_path, .. } => deploy_path,
			Self::Child { deploy_path, .. } => deploy_path,
		}
	}

	fn source_path(&self) -> &Path {
		match self {
			Self::File { source_path, .. } => source_path,
			Self::Child { source_path, .. } => source_path,
		}
	}

	fn path(&self) -> &Path {
		match self {
			Self::File { item, .. } => &item.path,
			Self::Child { path, .. } => path,
		}
	}

	fn priority(&self) -> Option<Priority> {
		match self {
			Self::File { item, .. } => item.priority,
			Self::Child { parent, .. } => parent.priority,
		}
	}

	fn merge_mode(&self) -> Option<MergeMode> {
		match self {
			Self::File { item, .. } => item.merge,
			Self::Child { parent, .. } => parent.merge,
		}
	}

	fn is_template(&self) -> bool {
		match self {
			Self::File { item, .. } => item.is_template(),
			Self::Child { parent, .. } => parent.is_template(),
		}
	}

	fn variables(&self) -> Option<&UserVars> {
		match self {
			Self::File { item, .. } => item.variables.as_ref(),
			Self::Child { parent, .. } => parent.variables.as_ref(),
		}
	}

	fn add_to_builder<S: Into<ItemStatus>>(self, builder: &mut DeploymentBuilder, status: S) {
		let status = status.into();

		match self {
			Self::File {
				item, deploy_path, ..
			} => builder.add_item(deploy_path, item, status),
			Self::Child {
				parent_deploy_path,
				deploy_path,
				..
			} => builder.add_child(deploy_path, parent_deploy_path.to_path_buf(), status),
		};
	}
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExecutorOptions {
	pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Executor<F> {
	options: ExecutorOptions,
	// called when same priority item exists and merge mode == Ask.
	// Gets called with item_source_path, item_deploy_path
	merge_ask_fn: F,
}

impl<F> Executor<F>
where
	F: Fn(&Path, &Path) -> Result<bool>,
{
	pub fn new(options: ExecutorOptions, f: F) -> Self {
		Self {
			options,
			merge_ask_fn: f,
		}
	}

	// TODO: remove err and just use builder.failed()???
	pub fn deploy(&self, source_path: PathBuf, mut profile: Profile) -> Result<Deployment> {
		// TODO: decide when deployment failed
		// TODO: check if it handles relative paths
		// TODO: function to resolve path (e.g. `~`, ...) OR function to resolve templated paths

		// FLOW:
		//	- get deployment path
		//	- check if item already deployed
		//	- YES:
		//		- compare priorities
		//		- LOWER/SAME: continue next item
		//		- HIGHER: skip file exists check
		//	- check if item exists
		//	- YES:
		//		- check merge operation
		//		- if merge operation == ASK
		//			- Run merge_ask_fn
		//			- FALSE: continue next item
		//	- check if template
		//	- YES: resolve template
		//	- IF FILE: write item
		//	- IF DIR: for each item in dir START AT TOP

		let target_path = &profile
			.target
			.clone()
			.unwrap_or_else(crate::get_target_path);
		let profiles_source_path = source_path.join("profiles");
		let items_source_path = source_path.join("items");

		let mut builder = Deployment::build();

		for hook in &profile.pre_hooks {
			log::info!("Executing pre hook: `{:?}`", hook);
			hook.execute(&profiles_source_path)
				.wrap_err("Failed to execute pre-hook")?;
		}

		let items = std::mem::take(&mut profile.items);

		for item in items.into_iter() {
			let _ = self.deploy_item(
				&mut builder,
				&items_source_path,
				target_path,
				&profile,
				item,
			)?;
		}

		for hook in &profile.post_hooks {
			log::info!("Executing post-hook: `{:?}`", hook);
			hook.execute(&profiles_source_path)
				.wrap_err("Failed to execute post-hook")?;
		}

		Ok(builder.success())
	}

	fn deploy_item(
		&self,
		builder: &mut DeploymentBuilder,
		items_source_path: &Path,
		target_path: &Path,
		profile: &Profile,
		item: Item,
	) -> Result<()> {
		let item_deploy_path = resolve_deployment_path(target_path, &item);
		let item_source_path = resolve_source_path(items_source_path, &item);

		log::debug!(
			"[{}] `{}` | `{}`",
			item.path.display(),
			item_source_path.display(),
			item_deploy_path.display()
		);

		// TODO: for now dont follow symlinks
		let metadata = match item_source_path.symlink_metadata() {
			Ok(metadata) => metadata,
			Err(err) => {
				log::warn!(
					"[{}] Failed to get metadata for item (`{}`)",
					item.path.display(),
					err
				);

				builder.add_item(item_deploy_path, item, err.into());

				return Ok(());
			}
		};

		if metadata.is_file() {
			let exec_item = ExecutorItem::File {
				item,
				source_path: item_source_path,
				deploy_path: item_deploy_path,
			};

			self.deploy_executor_item(builder, items_source_path, profile, exec_item)
		} else if metadata.is_dir() {
			self.deploy_dir(
				builder,
				items_source_path,
				target_path,
				profile,
				item,
				item_source_path,
				item_deploy_path,
			)
		} else {
			log::warn!(
				"[{}] Unsupported item type (`{:?}`)",
				item.path.display(),
				metadata.file_type()
			);

			builder.add_item(
				item_deploy_path,
				item,
				ItemStatus::failed(format!(
					"Unsupported item type `{:?}`",
					metadata.file_type()
				)),
			);

			Ok(())
		}
	}

	#[allow(clippy::too_many_arguments)]
	fn deploy_dir(
		&self,
		builder: &mut DeploymentBuilder,
		source_path: &Path,
		target_path: &Path,
		profile: &Profile,
		directory: Item,
		directory_source_path: PathBuf,
		directory_deploy_path: PathBuf,
	) -> Result<()> {
		// if no specific target path is set for the directory, use the root
		// target path as target. This will dump all children in the top level
		// path.
		let directory_deploy_path = match directory.target {
			None => target_path.to_path_buf(),
			Some(_) => directory_deploy_path,
		};

		let mut backlog: VecDeque<ReadDir> = VecDeque::new();

		match std::fs::create_dir_all(&directory_deploy_path) {
			Ok(_) => {}
			Err(err) => {
				log::warn!(
					"[{}] Failed to create directories (`{}`)",
					directory.path.display(),
					err
				);

				builder.add_item(directory_deploy_path, directory, err.into());

				return Ok(());
			}
		}

		let dents = match directory_source_path.read_dir() {
			Ok(read_dir) => read_dir,
			Err(err) => {
				log::warn!(
					"[{}] Failed to read directory (`{}`)",
					directory.path.display(),
					err
				);

				builder.add_item(directory_deploy_path, directory, err.into());

				return Ok(());
			}
		};

		backlog.push_back(dents);

		while !backlog.is_empty() {
			for dent in backlog.pop_front().expect("Backlog to have an item") {
				let dent = match dent {
					Ok(dent) => dent,
					Err(err) => {
						// TODO: handle better
						log::warn!("Failed to get dent: {}", err.to_string());
						continue;
					}
				};

				let child_source_path = dent.path();

				let child_path = match child_source_path.strip_prefix(&directory_source_path) {
					Ok(path) => path,
					Err(_) => {
						// TODO: handle better
						log::warn!(
							"[{}] Failed resolve child path (`{}`)",
							directory.path.display(),
							dent.path().display(),
						);

						continue;
					}
				};

				let child_deploy_path = directory_deploy_path.join(child_path);

				// TODO: for now dont follow symlinks
				let metadata = match dent.metadata() {
					Ok(metadata) => metadata,
					Err(err) => {
						log::warn!(
							"[{}] Failed to get metadata for child (`{}`)",
							child_path.display(),
							err
						);

						builder.add_child(
							child_deploy_path,
							directory_deploy_path.clone(),
							err.into(),
						);

						continue;
					}
				};

				if metadata.is_file() {
					let exec_item = ExecutorItem::Child {
						parent: &directory,
						parent_source_path: &directory_source_path,
						parent_deploy_path: &directory_deploy_path,
						path: child_path.to_path_buf(),
						source_path: child_source_path,
						deploy_path: child_deploy_path,
					};

					let _ = self.deploy_executor_item(builder, source_path, profile, exec_item)?;
				} else if metadata.is_dir() {
					let dents = match child_source_path.read_dir() {
						Ok(read_dir) => read_dir,
						Err(err) => {
							log::warn!(
								"[{}] Failed to read directory (`{}`)",
								child_path.display(),
								err
							);

							builder.add_child(
								child_deploy_path,
								directory_deploy_path.clone(),
								err.into(),
							);

							continue;
						}
					};

					backlog.push_back(dents);
				} else {
					log::warn!(
						"[{}] Unsupported item type (`{:?}`)",
						child_path.display(),
						metadata.file_type()
					);

					builder.add_child(
						child_deploy_path,
						directory_deploy_path.clone(),
						ItemStatus::failed(format!(
							"Unsupported item type `{:?}`",
							metadata.file_type()
						)),
					);
				}
			}
		}

		Ok(())
	}

	fn deploy_executor_item<'a>(
		&self,
		builder: &mut DeploymentBuilder,
		_source_path: &Path,
		profile: &Profile,
		exec_item: ExecutorItem<'a>,
	) -> Result<()> {
		// Check if there is an already deployed item at `deploy_path`.
		if let Some(other_priority) = builder.get_priority(exec_item.deploy_path()) {
			// Previously deployed item has higher priority; Skip current item.
			if other_priority > exec_item.priority() {
				log::info!(
					"[{}] Item with higher priority is already deployed",
					exec_item.path().display()
				);

				exec_item.add_to_builder(
					builder,
					ItemStatus::skipped("Item with higher priority is already deployed"),
				);

				return Ok(());
			}
		}

		if exec_item.deploy_path().exists() {
			// No previously deployed item at `deploy_path`. Check for merge.

			log::debug!(
				"[{}] Item already exists (`{}`)",
				exec_item.path().display(),
				exec_item.deploy_path().display()
			);

			match exec_item.merge_mode().unwrap_or_default() {
				MergeMode::Overwrite => {
					log::info!(
						"[{}] Overwritting existing item",
						exec_item.path().display()
					)
				}
				MergeMode::Keep => {
					log::info!("[{}] Skipping existing item", exec_item.path().display());

					exec_item.add_to_builder(
						builder,
						ItemStatus::skipped(format!(
							"Item already exists and merge mode is `{:?}`",
							MergeMode::Keep,
						)),
					);

					return Ok(());
				}
				MergeMode::Ask => {
					log::info!("[{}] Asking for action", exec_item.path().display());

					if !((self.merge_ask_fn)(exec_item.source_path(), exec_item.deploy_path())
						.wrap_err("Error evaluating user response")?)
					{
						log::info!("[{}] Merge was denied", exec_item.path().display());

						exec_item.add_to_builder(
							builder,
							ItemStatus::skipped("Item already exists and merge ask was denied"),
						);

						return Ok(());
					}
				}
			}
		}

		if let Some(parent) = exec_item.deploy_path().parent() {
			match std::fs::create_dir_all(parent) {
				Ok(_) => {}
				Err(err) => {
					log::warn!(
						"[{}] Failed to create directories (`{}`)",
						exec_item.path().display(),
						err
					);

					exec_item.add_to_builder(builder, err);

					return Ok(());
				}
			}
		}

		if exec_item.is_template() {
			let content = match std::fs::read_to_string(&exec_item.source_path()) {
				Ok(content) => content,
				Err(err) => {
					log::info!(
						"[{}] Failed to read source content",
						exec_item.path().display()
					);

					exec_item.add_to_builder(builder, err);

					return Ok(());
				}
			};

			let template = Template::parse(&content)
				.with_context(|| format!("File: {}", exec_item.source_path().display()))?;

			let content = template
				.fill(profile.variables.as_ref(), exec_item.variables())
				.with_context(|| format!("File: {}", exec_item.source_path().display()))?;

			if !self.options.dry_run {
				if let Err(err) = std::fs::write(&exec_item.deploy_path(), content.as_bytes()) {
					log::info!("[{}] Failed to write content", exec_item.path().display());

					exec_item.add_to_builder(builder, err);

					return Ok(());
				}
			}
		} else {
			// Allowed for readability
			#[allow(clippy::collapsible_else_if)]
			if !self.options.dry_run {
				if let Err(err) = std::fs::copy(&exec_item.source_path(), &exec_item.deploy_path())
				{
					log::info!("[{}] Failed to copy item", exec_item.path().display());

					exec_item.add_to_builder(builder, err);

					return Ok(());
				}
			}
		}

		log::info!(
			"[{}] Item successfully deployed",
			exec_item.path().display()
		);

		exec_item.add_to_builder(builder, ItemStatus::Success);

		Ok(())
	}
}

fn resolve_deployment_path(profile_target: &Path, item: &Item) -> PathBuf {
	match &item.target {
		Some(DeployTarget::Alias(alias)) => profile_target.join(alias),
		Some(DeployTarget::Path(path)) => path.clone(),
		None => profile_target.join(&item.path),
	}
}

fn resolve_source_path(source_path: &Path, item: &Item) -> PathBuf {
	source_path.join(&item.path)
}
