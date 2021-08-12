use std::collections::VecDeque;
use std::fs::ReadDir;
use std::path::{Path, PathBuf};

use color_eyre::eyre::Context;
use color_eyre::Result;

use super::deployment::{Deployment, DeploymentBuilder};
use crate::deploy::dotfile::DotfileStatus;
use crate::template::source::Source;
use crate::template::Template;
use crate::{DeployTarget, Dotfile, MergeMode, Profile};

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
		//	- check if dotfile already deployed
		//	- YES:
		//		- compare priorities
		//		- LOWER/SAME: continue next dotfile
		//		- HIGHER: skip file exists check
		//	- check if dotfile exists
		//	- YES:
		//		- check merge operation
		//		- if merge operation == ASK
		//			- Run merge_ask_fn
		//			- FALSE: continue next dotfile
		//	- check if template
		//	- YES: resolve template
		//	- IF FILE: write dotfile
		//	- IF DIR: for each dotfile in dir START AT TOP

		let profiles_source_path = source_path.join("profiles");
		let dofiles_source_path = source_path.join("dotfiles");

		let mut builder = Deployment::build();

		for hook in &profile.pre_hooks {
			log::info!("Executing pre hook: `{:?}`", hook);
			hook.execute(&profiles_source_path)
				.wrap_err("Failed to execute pre-hook")?;
		}

		let dotfiles = std::mem::take(&mut profile.dotfiles);

		for dotfile in dotfiles.into_iter() {
			let _ = self.deploy_dotfile(&mut builder, &dofiles_source_path, &profile, dotfile)?;
		}

		for hook in &profile.post_hooks {
			log::info!("Executing post-hook: `{:?}`", hook);
			hook.execute(&profiles_source_path)
				.wrap_err("Failed to execute post-hook")?;
		}

		Ok(builder.success())
	}

	fn deploy_dotfile(
		&self,
		builder: &mut DeploymentBuilder,
		dotfiles_source_path: &Path,
		profile: &Profile,
		dotfile: Dotfile,
	) -> Result<()> {
		// TODO: cleanup
		let dotfile_deploy_path = resolve_deployment_path(
			&profile
				.target
				.clone()
				.unwrap_or_else(crate::get_target_path),
			&dotfile,
		);
		let dotfile_source_path = resolve_source_path(dotfiles_source_path, &dotfile);

		log::debug!(
			"[{}] `{}` | `{}`",
			dotfile.path.display(),
			dotfile_source_path.display(),
			dotfile_deploy_path.display()
		);

		// TODO: for now dont follow symlinks
		let metadata = match dotfile_source_path.symlink_metadata() {
			Ok(metadata) => metadata,
			Err(err) => {
				log::error!(
					"[{}] Failed to get metadata for dotfile (`{}`)",
					dotfile.path.display(),
					err
				);

				builder.add_dotfile(dotfile_deploy_path, dotfile, err.into());

				return Ok(());
			}
		};

		if metadata.is_file() {
			self.deploy_file(
				builder,
				dotfiles_source_path,
				profile,
				dotfile,
				dotfile_source_path,
				dotfile_deploy_path,
			)
		} else if metadata.is_dir() {
			self.deploy_dir(
				builder,
				dotfiles_source_path,
				profile,
				dotfile,
				dotfile_source_path,
				dotfile_deploy_path,
			)
		} else {
			log::error!(
				"[{}] Unsupported dotfile type (`{:?}`)",
				dotfile.path.display(),
				metadata.file_type()
			);

			builder.add_dotfile(
				dotfile_deploy_path,
				dotfile,
				DotfileStatus::failed(format!(
					"Unsupported dotfile type `{:?}`",
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
		directory: Dotfile,
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
				log::error!(
					"[{}] Failed to create directories (`{}`)",
					directory.path.display(),
					err
				);

				builder.add_dotfile(directory_deploy_path, directory, err.into());

				return Ok(());
			}
		}

		let dents = match directory_source_path.read_dir() {
			Ok(read_dir) => read_dir,
			Err(err) => {
				log::error!(
					"[{}] Failed to read directory (`{}`)",
					directory.path.display(),
					err
				);

				builder.add_dotfile(directory_deploy_path, directory, err.into());

				return Ok(());
			}
		};

		backlog.push_back(dents);

		while !backlog.is_empty() {
			for dent in backlog.pop_front().expect("Backlog to have an dotfile") {
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
						log::error!(
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
						log::error!(
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
							log::error!(
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
					log::error!(
						"[{}] Unsupported dotfile type (`{:?}`)",
						child_path.display(),
						metadata.file_type()
					);

					builder.add_child(
						child_deploy_path,
						directory_deploy_path.clone(),
						DotfileStatus::failed(format!(
							"Unsupported dotfile type `{:?}`",
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
		directory: &Dotfile,
		_directory_source_path: &Path,
		directory_deploy_path: &Path,
		// relative path in source
		child_path: PathBuf,
		child_source_path: PathBuf,
		child_deploy_path: PathBuf,
	) -> Result<()> {
		// Check if there is an already deployed dotfile at `deploy_path`.
		if let Some(other_priority) = builder.get_priority(&child_deploy_path) {
			// Previously deployed dotfile has higher priority; Skip current dotfile.
			if other_priority > directory.priority {
				log::info!(
					"[{}] Dotfile with higher priority is already deployed",
					child_path.display()
				);

				builder.add_child(
					child_deploy_path,
					directory_deploy_path.to_path_buf(),
					DotfileStatus::skipped("Dotfile with higher priority is already deployed"),
				);

				return Ok(());
			}
		}

		if child_deploy_path.exists() {
			// No previously deployed dotfile at `deploy_path`. Check for merge.

			log::debug!(
				"[{}] Dotfile already exists (`{}`)",
				child_path.display(),
				child_deploy_path.display()
			);

			match directory.merge.unwrap_or_default() {
				MergeMode::Overwrite => {
					log::info!("[{}] Overwritting existing dotfile", child_path.display())
				}
				MergeMode::Keep => {
					log::info!("[{}] Skipping existing dotfile", child_path.display());

					builder.add_child(
						child_deploy_path,
						directory_deploy_path.to_path_buf(),
						DotfileStatus::skipped(format!(
							"Dotfile already exists and merge mode is `{:?}`",
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
							DotfileStatus::skipped(
								"Dotfile already exists and merge ask was denied",
							),
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
					log::error!(
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
					log::error!("[{}] Failed to read source content", child_path.display());
					builder.add_child(
						child_deploy_path,
						directory_deploy_path.to_path_buf(),
						err.into(),
					);
					return Ok(());
				}
			};

			let source = Source::file(&child_source_path, &content);
			let template = Template::parse(source)
				.with_context(|| format!("File: {}", child_source_path.display()))?;
			let content = template
				.resolve(profile.variables.as_ref(), directory.variables.as_ref())
				.with_context(|| format!("File: {}", child_source_path.display()))?;

			if !self.options.dry_run {
				if let Err(err) = std::fs::write(&child_deploy_path, content.as_bytes()) {
					log::error!("[{}] Failed to write content", child_path.display());
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
					log::error!("[{}] Failed to copy dotfile", child_path.display());
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
			DotfileStatus::Success,
		);

		log::info!("[{}] Dotfile successfully deployed", child_path.display());

		Ok(())
	}

	fn deploy_file(
		&self,
		builder: &mut DeploymentBuilder,
		_source_path: &Path,
		profile: &Profile,
		file: Dotfile,
		file_source_path: PathBuf,
		file_deploy_path: PathBuf,
	) -> Result<()> {
		// Check if there is an already deployed dotfile at `deploy_path`.
		if let Some(other_priority) = builder.get_priority(&file_deploy_path) {
			// Previously deployed dotfile has higher priority; Skip current dotfile.
			if other_priority > file.priority {
				log::info!(
					"[{}] Dotfile with higher priority is already deployed",
					file.path.display()
				);

				builder.add_dotfile(
					file_deploy_path,
					file,
					DotfileStatus::skipped("Dotfile with higher priority is already deployed"),
				);

				return Ok(());
			}
		}

		if file_deploy_path.exists() {
			// No previously deployed dotfile at `deploy_path`. Check for merge.

			log::debug!(
				"[{}] Dotfile already exists (`{}`)",
				file.path.display(),
				file_deploy_path.display()
			);

			match file.merge.unwrap_or_default() {
				MergeMode::Overwrite => {
					log::info!("[{}] Overwritting existing dotfile", file.path.display())
				}
				MergeMode::Keep => {
					log::info!("[{}] Skipping existing dotfile", file.path.display());

					builder.add_dotfile(
						file_deploy_path,
						file,
						DotfileStatus::skipped(format!(
							"Dotfile already exists and merge mode is `{:?}`",
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

						builder.add_dotfile(
							file_deploy_path,
							file,
							DotfileStatus::skipped(
								"Dotfile already exists and merge ask was denied",
							),
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
					log::error!(
						"[{}] Failed to create directories (`{}`)",
						file.path.display(),
						err
					);

					builder.add_dotfile(file_deploy_path, file, err.into());

					return Ok(());
				}
			}
		}

		if file.is_template() {
			let content = match std::fs::read_to_string(&file_source_path) {
				Ok(content) => content,
				Err(err) => {
					log::error!("[{}] Failed to read source content", file.path.display());
					builder.add_dotfile(file_deploy_path, file, err.into());
					return Ok(());
				}
			};

			let source = Source::file(&file_source_path, &content);
			let template = Template::parse(source)
				.with_context(|| format!("File: {}", file_source_path.display()))?;
			let content = template
				.resolve(profile.variables.as_ref(), file.variables.as_ref())
				.with_context(|| format!("File: {}", file_source_path.display()))?;

			if !self.options.dry_run {
				// TODO: do template transform
				if let Err(err) = std::fs::write(&file_deploy_path, content.as_bytes()) {
					log::error!("[{}] Failed to write content", file.path.display());
					builder.add_dotfile(file_deploy_path, file, err.into());
					return Ok(());
				}
			}
		} else {
			// Allowed for readability
			#[allow(clippy::collapsible_else_if)]
			if !self.options.dry_run {
				if let Err(err) = std::fs::copy(&file_source_path, &file_deploy_path) {
					log::error!("[{}] Failed to copy dotfile", file.path.display());
					builder.add_dotfile(file_deploy_path, file, err.into());
					return Ok(());
				}
			}
		}

		log::info!("[{}] Dotfile successfully deployed", file.path.display());
		builder.add_dotfile(file_deploy_path, file, DotfileStatus::Success);

		Ok(())
	}
}

fn resolve_deployment_path(profile_target: &Path, dotfile: &Dotfile) -> PathBuf {
	match &dotfile.target {
		Some(DeployTarget::Alias(alias)) => profile_target.join(alias),
		Some(DeployTarget::Path(path)) => path.clone(),
		None => profile_target.join(&dotfile.path),
	}
}

fn resolve_source_path(source_path: &Path, dotfile: &Dotfile) -> PathBuf {
	source_path.join(&dotfile.path)
}
