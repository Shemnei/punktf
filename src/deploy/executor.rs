use std::collections::VecDeque;
use std::fs::ReadDir;
use std::path::{Path, PathBuf};

use color_eyre::eyre::Context;
use color_eyre::Result;

use super::deployment::{Deployment, DeploymentBuilder};
use crate::deploy::item::ItemStatus;
use crate::template::Template;
use crate::{DeployTarget, Item, MergeMode, Profile};

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExecutorOptions {
	pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Executor<F> {
	options: ExecutorOptions,
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

	pub fn deploy(&self, source_path: PathBuf, mut profile: Profile) -> Result<Deployment> {
		// TODO: decide when deployment failed
		// TODO: check if it handles relative paths
		// TODO: merge code from deploy_file/deploy_child
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

		let mut builder = Deployment::build();

		for hook in &profile.pre_hooks {
			log::info!("Executing pre hook: `{:?}`", hook);
			hook.execute().wrap_err("Failed to execute pre-hook")?;
		}

		let items = std::mem::take(&mut profile.items);

		for item in items.into_iter() {
			let _ = self.deploy_item(&mut builder, &source_path, &profile, item)?;
		}

		for hook in &profile.post_hooks {
			log::info!("Executing post-hook: `{:?}`", hook);
			hook.execute().wrap_err("Failed to execute post-hook")?;
		}

		Ok(builder.success())
	}

	fn deploy_item(
		&self,
		builder: &mut DeploymentBuilder,
		source_path: &Path,
		profile: &Profile,
		item: Item,
	) -> Result<()> {
		// TODO: cleanup
		let item_deploy_path = resolve_deployment_path(
			&profile
				.target
				.clone()
				.unwrap_or_else(crate::get_target_path),
			&item,
		);
		let item_source_path = resolve_source_path(source_path, &item);

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
			self.deploy_file(
				builder,
				source_path,
				profile,
				item,
				item_source_path,
				item_deploy_path,
			)
		} else if metadata.is_dir() {
			self.deploy_dir(
				builder,
				source_path,
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

	fn deploy_dir(
		&self,
		builder: &mut DeploymentBuilder,
		source_path: &Path,
		profile: &Profile,
		directory: Item,
		directory_source_path: PathBuf,
		directory_deploy_path: PathBuf,
	) -> Result<()> {
		let directory_deploy_path = match directory.target {
			None => profile
				.target
				.clone()
				.unwrap_or_else(crate::get_target_path),
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
						log::error!("{}", err.to_string());
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
					let _ = self.deploy_child(
						builder,
						source_path,
						profile,
						&directory,
						&directory_source_path,
						&directory_deploy_path,
						child_path.to_path_buf(),
						child_source_path,
						child_deploy_path,
					)?;
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

	// Allowed as the final args are not yet been decided
	#[allow(clippy::too_many_arguments)]
	fn deploy_child(
		&self,
		builder: &mut DeploymentBuilder,
		_source_path: &Path,
		profile: &Profile,
		directory: &Item,
		_directory_source_path: &Path,
		directory_deploy_path: &Path,
		// relative path in source
		child_path: PathBuf,
		child_source_path: PathBuf,
		child_deploy_path: PathBuf,
	) -> Result<()> {
		// Check if there is an already deployed item at `deploy_path`.
		if let Some(other_priority) = builder.get_priority(&child_deploy_path) {
			// Previously deployed item has higher priority; Skip current item.
			if other_priority > directory.priority {
				log::info!(
					"[{}] Item with higher priority is already deployed",
					child_path.display()
				);

				builder.add_child(
					child_deploy_path,
					directory_deploy_path.to_path_buf(),
					ItemStatus::skipped("Item with higher priority is already deployed"),
				);

				return Ok(());
			}
		}

		if child_deploy_path.exists() {
			// No previously deployed item at `deploy_path`. Check for merge.

			log::debug!(
				"[{}] Item already exists (`{}`)",
				child_path.display(),
				child_deploy_path.display()
			);

			match directory.merge.unwrap_or_default() {
				MergeMode::Overwrite => {
					log::info!("[{}] Overwritting existing item", child_path.display())
				}
				MergeMode::Keep => {
					log::info!("[{}] Skipping existing item", child_path.display());

					builder.add_child(
						child_deploy_path,
						directory_deploy_path.to_path_buf(),
						ItemStatus::skipped(format!(
							"Item already exists and merge mode is `{:?}`",
							MergeMode::Keep,
						)),
					);

					return Ok(());
				}
				MergeMode::Ask => {
					log::info!("[{}] Asking for action", child_path.display());

					if !((self.merge_ask_fn)(&child_deploy_path, &child_source_path)
						.wrap_err("Error evaluating user response")?)
					{
						log::info!("[{}] Merge was denied", child_path.display());

						builder.add_child(
							child_deploy_path,
							directory_deploy_path.to_path_buf(),
							ItemStatus::skipped("Item already exists and merge ask was denied"),
						);

						return Ok(());
					}
				}
			}
		}

		if let Some(parent) = child_deploy_path.parent() {
			match std::fs::create_dir_all(parent) {
				Ok(_) => {}
				Err(err) => {
					log::warn!(
						"[{}] Failed to create directories (`{}`)",
						child_path.display(),
						err
					);

					builder.add_child(
						child_deploy_path,
						directory_deploy_path.to_path_buf(),
						err.into(),
					);

					return Ok(());
				}
			}
		}

		if directory.is_template() {
			let content = match std::fs::read_to_string(&child_source_path) {
				Ok(content) => content,
				Err(err) => {
					log::info!("[{}] Failed to read source content", child_path.display());
					builder.add_child(
						child_deploy_path,
						directory_deploy_path.to_path_buf(),
						err.into(),
					);
					return Ok(());
				}
			};

			let template = Template::parse(&content)
				.with_context(|| format!("File: {}", child_source_path.display()))?;
			let content = template
				.fill(profile.variables.as_ref(), directory.variables.as_ref())
				.with_context(|| format!("File: {}", child_source_path.display()))?;

			if !self.options.dry_run {
				if let Err(err) = std::fs::write(&child_deploy_path, content.as_bytes()) {
					log::info!("[{}] Failed to write content", child_path.display());
					builder.add_child(
						child_deploy_path,
						directory_deploy_path.to_path_buf(),
						err.into(),
					);
					return Ok(());
				}
			}
		} else {
			// Allowed for readability
			#[allow(clippy::collapsible_else_if)]
			if !self.options.dry_run {
				if let Err(err) = std::fs::copy(&child_source_path, &child_deploy_path) {
					log::info!("[{}] Failed to copy item", child_path.display());
					builder.add_child(
						child_deploy_path,
						directory_deploy_path.to_path_buf(),
						err.into(),
					);
					return Ok(());
				}
			}
		}

		builder.add_child(
			child_deploy_path,
			directory_deploy_path.to_path_buf(),
			ItemStatus::Success,
		);

		log::info!("[{}] Item successfully deployed", child_path.display());

		Ok(())
	}

	fn deploy_file(
		&self,
		builder: &mut DeploymentBuilder,
		_source_path: &Path,
		profile: &Profile,
		file: Item,
		file_source_path: PathBuf,
		file_deploy_path: PathBuf,
	) -> Result<()> {
		// Check if there is an already deployed item at `deploy_path`.
		if let Some(other_priority) = builder.get_priority(&file_deploy_path) {
			// Previously deployed item has higher priority; Skip current item.
			if other_priority > file.priority {
				log::info!(
					"[{}] Item with higher priority is already deployed",
					file.path.display()
				);

				builder.add_item(
					file_deploy_path,
					file,
					ItemStatus::skipped("Item with higher priority is already deployed"),
				);

				return Ok(());
			}
		}

		if file_deploy_path.exists() {
			// No previously deployed item at `deploy_path`. Check for merge.

			log::debug!(
				"[{}] Item already exists (`{}`)",
				file.path.display(),
				file_deploy_path.display()
			);

			match file.merge.unwrap_or_default() {
				MergeMode::Overwrite => {
					log::info!("[{}] Overwritting existing item", file.path.display())
				}
				MergeMode::Keep => {
					log::info!("[{}] Skipping existing item", file.path.display());

					builder.add_item(
						file_deploy_path,
						file,
						ItemStatus::skipped(format!(
							"Item already exists and merge mode is `{:?}`",
							MergeMode::Keep,
						)),
					);

					return Ok(());
				}
				MergeMode::Ask => {
					log::info!("[{}] Asking for action", file_deploy_path.display());

					if !((self.merge_ask_fn)(&file_deploy_path, &file_source_path)
						.wrap_err("Error evaluating user response")?)
					{
						log::info!("[{}] Merge was denied", file.path.display());

						builder.add_item(
							file_deploy_path,
							file,
							ItemStatus::skipped("Item already exists and merge ask was denied"),
						);

						return Ok(());
					}
				}
			}
		}

		if let Some(parent) = file_deploy_path.parent() {
			match std::fs::create_dir_all(parent) {
				Ok(_) => {}
				Err(err) => {
					log::warn!(
						"[{}] Failed to create directories (`{}`)",
						file.path.display(),
						err
					);

					builder.add_item(file_deploy_path, file, err.into());

					return Ok(());
				}
			}
		}

		if file.is_template() {
			let content = match std::fs::read_to_string(&file_source_path) {
				Ok(content) => content,
				Err(err) => {
					log::info!("[{}] Failed to read source content", file.path.display());
					builder.add_item(file_deploy_path, file, err.into());
					return Ok(());
				}
			};

			let template = Template::parse(&content)
				.with_context(|| format!("File: {}", file_source_path.display()))?;
			let content = template
				.fill(profile.variables.as_ref(), file.variables.as_ref())
				.with_context(|| format!("File: {}", file_source_path.display()))?;

			if !self.options.dry_run {
				// TODO: do template transform
				if let Err(err) = std::fs::write(&file_deploy_path, content.as_bytes()) {
					log::info!("[{}] Failed to write content", file.path.display());
					builder.add_item(file_deploy_path, file, err.into());
					return Ok(());
				}
			}
		} else {
			// Allowed for readability
			#[allow(clippy::collapsible_else_if)]
			if !self.options.dry_run {
				if let Err(err) = std::fs::copy(&file_source_path, &file_deploy_path) {
					log::info!("[{}] Failed to copy item", file.path.display());
					builder.add_item(file_deploy_path, file, err.into());
					return Ok(());
				}
			}
		}

		log::info!("[{}] Item successfully deployed", file.path.display());
		builder.add_item(file_deploy_path, file, ItemStatus::Success);

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
