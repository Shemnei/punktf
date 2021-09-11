//! Everting needed to deploy a [profile](`crate::profile::Profile`).

use std::path::{Path, PathBuf};

use color_eyre::eyre::Context;
use color_eyre::Result;

use super::deployment::{Deployment, DeploymentBuilder};
use crate::deploy::dotfile::DotfileStatus;
use crate::profile::LayeredProfile;
use crate::template::source::Source;
use crate::template::Template;
use crate::transform::{ContentTransformer, Transform as _};
use crate::variables::Variables;
use crate::{Dotfile, MergeMode, Priority, PunktfSource};

/// An enum to be generic over both a "real" dotfile and a child of a directory
/// dotfile.
enum ExecutorDotfile<'a> {
	/// A "real" dotfile.
	File {
		/// The dotfile.
		dotfile: Dotfile,

		/// The absolute source path of the dotfile.
		source_path: PathBuf,

		/// The absolute deploy path of the dotfile.
		deploy_path: PathBuf,
	},
	/// A child of a directory dotfile.
	Child {
		/// The directory dotfile this child stems from.
		parent: &'a Dotfile,

		/// The absolute source path of the directory dotfile this child stems
		/// from.
		parent_source_path: &'a Path,

		/// The absolute deploy path of the directory dotfile this child stems
		/// from.
		parent_deploy_path: &'a Path,

		/// Relative path in
		/// [PunktfSource::dotfiles](`crate::PunktfSource::dotfiles`)
		/// (equivalent to [Dotfile::path](`crate::Dotfile::path`)).
		path: PathBuf,

		/// The absolute source path of the child.
		source_path: PathBuf,

		/// The absolute deploy path of the child.
		deploy_path: PathBuf,
	},
}

impl<'a> ExecutorDotfile<'a> {
	/// Returns the absolute deploy path.
	fn deploy_path(&self) -> &Path {
		match self {
			Self::File { deploy_path, .. } => deploy_path,
			Self::Child { deploy_path, .. } => deploy_path,
		}
	}

	/// Returns the absolute source path.
	fn source_path(&self) -> &Path {
		match self {
			Self::File { source_path, .. } => source_path,
			Self::Child { source_path, .. } => source_path,
		}
	}

	/// Returns the relative path in [PunktfSource::dotfiles](`crate::PunktfSource::dotfiles`).
	fn path(&self) -> &Path {
		match self {
			Self::File { dotfile, .. } => &dotfile.path,
			Self::Child { path, .. } => path,
		}
	}

	/// Returns the priority.
	///
	/// If it is a child, the of the directory dotfile is used.
	const fn priority(&self) -> Option<Priority> {
		match self {
			Self::File { dotfile, .. } => dotfile.priority,
			Self::Child { parent, .. } => parent.priority,
		}
	}

	/// Returns the merge mode.
	///
	/// If it is a child, the value of the directory dotfile is used.
	const fn merge_mode(&self) -> Option<MergeMode> {
		match self {
			Self::File { dotfile, .. } => dotfile.merge,
			Self::Child { parent, .. } => parent.merge,
		}
	}

	/// Returns whether this is a template.
	///
	/// If it is a child, the value of the directory dotfile is used.
	fn is_template(&self) -> bool {
		match self {
			Self::File { dotfile, .. } => dotfile.is_template(),
			Self::Child { parent, .. } => parent.is_template(),
		}
	}

	/// Returns the [dotfile variables][`crate::Dotfile::variables`].
	///
	/// If it is a child, the value of the directory dotfile is used.
	const fn variables(&self) -> Option<&Variables> {
		match self {
			Self::File { dotfile, .. } => dotfile.variables.as_ref(),
			Self::Child { parent, .. } => parent.variables.as_ref(),
		}
	}

	/// Returns the [dotfile transformers][`crate::Dotfile::transformers`].
	///
	/// If it is a child, the value of the directory dotfile is used.
	fn transformers(&self) -> &[ContentTransformer] {
		match self {
			Self::File { dotfile, .. } => &dotfile.transformers,
			Self::Child { parent, .. } => &parent.transformers,
		}
	}

	/// Adds the item to a deployment builder with the given `status`.
	fn add_to_builder<S: Into<DotfileStatus>>(self, builder: &mut DeploymentBuilder, status: S) {
		let status = status.into();

		let resolved_deploy_path = self
			.deploy_path()
			.canonicalize()
			.unwrap_or_else(|_| self.deploy_path().to_path_buf());

		match self {
			Self::File { dotfile, .. } => {
				builder.add_dotfile(resolved_deploy_path, dotfile, status)
			}
			Self::Child {
				parent_deploy_path, ..
			} => builder.add_child(
				resolved_deploy_path,
				parent_deploy_path.to_path_buf(),
				status,
			),
		};
	}
}

/// Configuration options for the [`Executor`].
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExecutorOptions {
	/// If this flag is set, it will prevent any write operations from occurring
	/// during the deployment.
	///
	/// This includes write, copy and directory creation operations.
	pub dry_run: bool,
}

/// The executor is responsible for deploying a
/// [profile](`crate::profile::Profile`).
///
/// This includes checking for merge conflicts, resolving children of a
/// directory dotfile, parsing and resolving of templates and the actual
/// writing of the dotfile to the target destination.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Executor<F> {
	/// Configuration options for the executor.
	options: ExecutorOptions,

	/// This function gets called when a dotfile at the target destination
	/// already exists and the merge mode is
	/// [MergeMode::Ask](`crate::MergeMode::Ask`).
	///
	/// The arguments for the function are (dotfile_source_path,
	/// dotfile_deploy_path).
	merge_ask_fn: F,
}

impl<F> Executor<F>
where
	F: Fn(&Path, &Path) -> Result<bool>,
{
	/// Creates a new executor with the given `options`.
	pub fn new(options: ExecutorOptions, f: F) -> Self {
		Self {
			options,
			merge_ask_fn: f,
		}
	}

	/// Tries to deploy the given `profile`.
	///
	/// # Errors
	///
	/// Only hard errors will be returned as error, everthing else will be
	/// recorded in the [Deployment](`super::deployment::Deployment`) on a
	/// dotfile level.
	pub fn deploy(&self, source: PunktfSource, profile: &LayeredProfile) -> Result<Deployment> {
		// General flow:
		//	- get deployment path
		//	- check if dotfile already deployed
		//	- YES:
		//		- compare priorities
		//		- LOWER: continue next dotfile
		//		- SAME/HIGHER: next step
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

		let target_path = &profile
			.target_path()
			.expect("No target path set")
			.to_path_buf();

		let mut builder = Deployment::build();

		for hook in profile.pre_hooks() {
			log::info!("Executing pre-hook: {}", hook.command());
			// No files are deployed yet, meaning if an error during hook
			// execution occurs it will return with an error instead of just
			// logging it.

			hook.execute(source.profiles())
				.wrap_err("Failed to execute pre-hook")?;
		}

		for dotfile in profile.dotfiles().cloned() {
			log::debug!("Deploying dotfile: {}", dotfile.path.display());
			let _ = self.deploy_dotfile(&mut builder, &source, target_path, profile, dotfile)?;
		}

		for hook in profile.post_hooks() {
			log::info!("Executing post-hook: {}", hook.command());
			if let Err(err) = hook.execute(source.profiles()) {
				log::error!("Failed to execute post-hook ({})", err);
			}
		}

		Ok(builder.finish())
	}

	/// Resolves all necessary paths to deploy the dotfile and tries to deploys it.
	///
	/// If the dotfile is a directory, [`Executor::deploy_dir`] will be called,
	/// otherwise [`Executor::deploy_executor_dotfile`] will be called.
	///
	/// Gets called for each dotfile defined by a
	/// [profile](`crate::profile::Profile`).
	fn deploy_dotfile(
		&self,
		builder: &mut DeploymentBuilder,
		source: &PunktfSource,
		target_path: &Path,
		profile: &LayeredProfile,
		dotfile: Dotfile,
	) -> Result<()> {
		let dotfile_deploy_path = resolve_deployment_path(target_path, &dotfile);

		let dotfile_source_path = match resolve_source_path(source.dotfiles(), &dotfile) {
			Ok(dotfile_source_path) => dotfile_source_path,
			Err(err) => {
				log::error!(
					"{}: Failed to resolve dotfile source path ({})",
					dotfile.path.display(),
					err,
				);

				builder.add_dotfile(
					dotfile_deploy_path,
					dotfile,
					DotfileStatus::failed(format!("Failed to resolve source path: {}", err)),
				);

				return Ok(());
			}
		};

		log::debug!(
			"{}: Source: `{}` Target: `{}`",
			dotfile.path.display(),
			dotfile_source_path.display(),
			dotfile_deploy_path.display()
		);

		// For now dont follow symlinks (`metadata()` would get the metadata of the target of a
		// link).
		let metadata = match dotfile_source_path.symlink_metadata() {
			Ok(metadata) => metadata,
			Err(err) => {
				log::error!(
					"{}: Failed to get metadata for dotfile ({})",
					dotfile.path.display(),
					err
				);

				builder.add_dotfile(
					dotfile_deploy_path,
					dotfile,
					DotfileStatus::failed(format!("Failed to read metadata: {}", err)),
				);

				return Ok(());
			}
		};

		if metadata.is_file() {
			let exec_dotfile = ExecutorDotfile::File {
				dotfile,
				source_path: dotfile_source_path,
				deploy_path: dotfile_deploy_path,
			};

			self.deploy_executor_dotfile(builder, source, profile, exec_dotfile)
		} else if metadata.is_dir() {
			self.deploy_dir(
				builder,
				source,
				target_path,
				profile,
				dotfile,
				dotfile_source_path,
				dotfile_deploy_path,
			)
		} else {
			log::error!(
				"{}: Unsupported dotfile type ({:?})",
				dotfile.path.display(),
				metadata.file_type()
			);

			builder.add_dotfile(
				dotfile_deploy_path,
				dotfile,
				DotfileStatus::failed(format!(
					"Unsupported dotfile type: {:?}",
					metadata.file_type()
				)),
			);

			Ok(())
		}
	}

	/// Deploys a dotfile directory by iterating over all children contained
	/// wihwithin the directory and it's subdirectories and deploying them.
	///
	/// This will call [`Executor::deploy_executor_dotfile`] for each child
	/// found.
	#[allow(clippy::too_many_arguments)]
	fn deploy_dir(
		&self,
		builder: &mut DeploymentBuilder,
		source: &PunktfSource,
		target_path: &Path,
		profile: &LayeredProfile,
		directory: Dotfile,
		directory_source_path: PathBuf,
		directory_deploy_path: PathBuf,
	) -> Result<()> {
		// If no specific target path is set for the directory, use the root
		// target path as target. This will dump all children in the top level
		// path.
		let directory_deploy_path = if directory.rename.is_some() {
			directory_deploy_path
		} else {
			directory
				.overwrite_target
				.clone()
				.unwrap_or_else(|| target_path.to_path_buf())
		};

		if !self.options.dry_run {
			match std::fs::create_dir_all(&directory_deploy_path) {
				Ok(_) => {}
				Err(err) => {
					log::error!(
						"{}: Failed to create directory ({})",
						directory.path.display(),
						err
					);

					builder.add_dotfile(
						directory_deploy_path,
						directory,
						DotfileStatus::failed(format!("Failed to create directory: {}", err)),
					);

					return Ok(());
				}
			}
		}

		for dent in walkdir::WalkDir::new(&directory_source_path) {
			let dent = match dent {
				Ok(dent) => dent,
				Err(err) => {
					log::error!(
						"{}: Failed to get directory entry ({})",
						directory.path.display(),
						err.to_string()
					);

					continue;
				}
			};

			let child_source_path = dent.path();

			let child_path = match child_source_path.strip_prefix(&directory_source_path) {
				Ok(path) => path,
				Err(_) => {
					log::error!(
						"{}: Failed resolve child path ({})",
						directory.path.display(),
						dent.path().display(),
					);

					continue;
				}
			};

			let child_deploy_path = directory_deploy_path.join(child_path);

			// For now don't follow symlinks (`metadata()` would get the metadata of the target of a
			// link).
			let metadata = match dent.metadata() {
				Ok(metadata) => metadata,
				Err(err) => {
					log::error!(
						"{}: Failed to get metadata for child ({})",
						child_path.display(),
						err
					);

					builder.add_child(
						child_deploy_path,
						directory_deploy_path.clone(),
						DotfileStatus::failed(format!("Failed to read metadata: {}", err)),
					);

					continue;
				}
			};

			if metadata.is_file() {
				let exec_dotfile = ExecutorDotfile::Child {
					parent: &directory,
					parent_source_path: &directory_source_path,
					parent_deploy_path: &directory_deploy_path,
					path: child_path.to_path_buf(),
					source_path: child_source_path.to_path_buf(),
					deploy_path: child_deploy_path,
				};

				let _ = self.deploy_executor_dotfile(builder, source, profile, exec_dotfile)?;
			} else if metadata.is_dir() {
				if !self.options.dry_run {
					match std::fs::create_dir_all(&child_deploy_path) {
						Ok(_) => {}
						Err(err) => {
							log::error!(
								"{}: Failed to create directory ({})",
								child_path.display(),
								err
							);

							builder.add_child(
								child_deploy_path,
								directory_deploy_path,
								DotfileStatus::failed(format!(
									"Failed to create directory: {}",
									err
								)),
							);

							return Ok(());
						}
					}
				}
			} else {
				log::error!(
					"{}: Unsupported dotfile file type ({:?})",
					child_path.display(),
					metadata.file_type()
				);

				builder.add_child(
					child_deploy_path,
					directory_deploy_path.clone(),
					DotfileStatus::failed(format!(
						"Unsupported dotfile file type: {:?}",
						metadata.file_type()
					)),
				);
			}
		}

		if !self.options.dry_run {
			// Only try to resolve when not in dry_run as the directory could
			// not exists and would not be created when in dry_run.
			match directory_deploy_path.canonicalize() {
				Ok(directory_deploy_path) => {
					builder.add_dotfile(directory_deploy_path, directory, DotfileStatus::Success);
				}
				Err(_) => {
					builder.add_dotfile(
						directory_deploy_path,
						directory,
						DotfileStatus::failed("Failed to canonicalize path"),
					);
				}
			};
		} else {
			builder.add_dotfile(directory_deploy_path, directory, DotfileStatus::Success);
		}

		Ok(())
	}

	/// This function does the actual deploying of a dotfile or a child of a
	/// dotfile.
	///
	/// It first checks if a dotfile with a higher priority was already
	/// deployed and if so returns early.
	/// After that it checks the merge mode and executes the appropriate action
	/// for it.
	/// After that it will parse and resolve the template if it is one and
	/// lastly will write the content to the deploy path.
	fn deploy_executor_dotfile<'a>(
		&self,
		builder: &mut DeploymentBuilder,
		source: &PunktfSource,
		profile: &LayeredProfile,
		exec_dotfile: ExecutorDotfile<'a>,
	) -> Result<()> {
		if !exec_dotfile.source_path().starts_with(source.dotfiles()) {
			log::warn!(
				"{}: Dotfile is not contained within the source `dotfiles` directory. This item \
				 will probably also be deployed \"above\" (in the directory tree) the target \
				 directory.",
				exec_dotfile.path().display()
			);
		}

		// Check if there is an already deployed dotfile at `deploy_path`.
		if let Some(other_priority) = builder.get_priority(exec_dotfile.deploy_path()) {
			// Previously deployed dotfile has higher priority; Skip current dotfile.
			if other_priority > exec_dotfile.priority() {
				log::info!(
					"{}: Dotfile with higher priority is already deployed",
					exec_dotfile.path().display()
				);

				exec_dotfile.add_to_builder(
					builder,
					DotfileStatus::skipped("Dotfile with higher priority is already deployed"),
				);

				return Ok(());
			}
		}

		if exec_dotfile.deploy_path().exists() {
			// No previously deployed dotfile at `deploy_path`. Check for merge.

			log::debug!(
				"{}: Dotfile already exists ({})",
				exec_dotfile.path().display(),
				exec_dotfile.deploy_path().display()
			);

			match exec_dotfile.merge_mode().unwrap_or_default() {
				MergeMode::Overwrite => {
					log::info!(
						"{}: Overwritting existing dotfile",
						exec_dotfile.path().display()
					)
				}
				MergeMode::Keep => {
					log::info!(
						"{}: Skipping existing dotfile",
						exec_dotfile.path().display()
					);

					exec_dotfile.add_to_builder(
						builder,
						DotfileStatus::skipped(format!(
							"Dotfile already exists and merge mode is {:?}",
							MergeMode::Keep,
						)),
					);

					return Ok(());
				}
				MergeMode::Ask => {
					log::info!("{}: Asking for action", exec_dotfile.path().display());

					let should_deploy = match (self.merge_ask_fn)(
						exec_dotfile.source_path(),
						exec_dotfile.deploy_path(),
					)
					.wrap_err("Error evaluating user response")
					{
						Ok(should_deploy) => should_deploy,
						Err(err) => {
							log::error!(
								"{}: Failed to execute ask function ({})",
								exec_dotfile.path().display(),
								err
							);

							exec_dotfile.add_to_builder(
								builder,
								DotfileStatus::failed(format!(
									"Failed to execute merge ask function: {}",
									err.to_string()
								)),
							);

							return Ok(());
						}
					};

					if !should_deploy {
						log::info!("{}: Merge was denied", exec_dotfile.path().display());

						exec_dotfile.add_to_builder(
							builder,
							DotfileStatus::skipped(
								"Dotfile already exists and merge ask was denied",
							),
						);

						return Ok(());
					}
				}
			}
		}

		if let Some(parent) = exec_dotfile.deploy_path().parent() {
			if !self.options.dry_run {
				match std::fs::create_dir_all(parent) {
					Ok(_) => {}
					Err(err) => {
						log::error!(
							"{}: Failed to create directory ({})",
							exec_dotfile.path().display(),
							err
						);

						exec_dotfile.add_to_builder(
							builder,
							DotfileStatus::failed(format!(
								"Failed to create parent directory: {}",
								err
							)),
						);

						return Ok(());
					}
				}
			}
		}

		// Fast Path.
		if !exec_dotfile.is_template()
			&& exec_dotfile.transformers().is_empty()
			&& profile.transformers_origin().is_empty()
		{
			// File is no template and no transformers are specified. This means
			// we can take the fast path of just copying via the filesystem.

			// Allowed for readability
			#[allow(clippy::collapsible_else_if)]
			if !self.options.dry_run {
				if let Err(err) =
					std::fs::copy(&exec_dotfile.source_path(), &exec_dotfile.deploy_path())
				{
					log::info!("{}: Failed to copy dotfile", exec_dotfile.path().display());

					exec_dotfile.add_to_builder(
						builder,
						DotfileStatus::failed(format!("Failed to copy: {}", err)),
					);

					return Ok(());
				}
			}
		} else {
			let mut content = if exec_dotfile.is_template() {
				let content = match std::fs::read_to_string(&exec_dotfile.source_path()) {
					Ok(content) => content,
					Err(err) => {
						log::info!(
							"{}: Failed to read dotfile source content",
							exec_dotfile.path().display()
						);

						exec_dotfile.add_to_builder(
							builder,
							DotfileStatus::failed(format!(
								"Failed to read dotfile source content: {}",
								err
							)),
						);

						return Ok(());
					}
				};

				let source = Source::file(exec_dotfile.source_path(), &content);

				let template = match Template::parse(source)
					.with_context(|| format!("File: {}", exec_dotfile.source_path().display()))
				{
					Ok(template) => template,
					Err(err) => {
						log::error!(
							"{}: Failed to parse template",
							exec_dotfile.path().display()
						);

						exec_dotfile.add_to_builder(
							builder,
							DotfileStatus::failed(format!("Failed to parse template: {}", err)),
						);

						return Ok(());
					}
				};

				let content = match template
					.resolve(Some(profile.variables()), exec_dotfile.variables())
					.with_context(|| format!("File: {}", exec_dotfile.source_path().display()))
				{
					Ok(template) => template,
					Err(err) => {
						log::error!(
							"{}: Failed to resolve template",
							exec_dotfile.path().display()
						);

						exec_dotfile.add_to_builder(
							builder,
							DotfileStatus::failed(format!("Failed to resolve template: {}", err)),
						);

						return Ok(());
					}
				};

				log::trace!("{}: Resolved:\n{}", exec_dotfile.path().display(), content);

				content
			} else {
				// We have some transformers to run on the content and thus can not straight copy the
				// contents.
				match std::fs::read_to_string(&exec_dotfile.source_path()) {
					Ok(content) => content,
					Err(err) => {
						log::info!("{}: Failed to copy dotfile", exec_dotfile.path().display());

						exec_dotfile.add_to_builder(
							builder,
							DotfileStatus::failed(format!("Failed to copy: {}", err)),
						);

						return Ok(());
					}
				}
			};

			// Copy so we exec_dotfile is not referenced by this in case an error occurs.
			let exec_transformers: Vec<_> = exec_dotfile.transformers().iter().copied().collect();

			// Apply transformers.
			// Order:
			//   - Dotfile transformers
			//   - Then profile transformers
			for transformer in exec_transformers.iter().chain(profile.transformers()) {
				content = match transformer.transform(content) {
					Ok(content) => content,
					Err(err) => {
						log::info!(
							"{}: Failed to apply content transformer `{}`: `{}`",
							exec_dotfile.path().display(),
							transformer,
							err
						);

						exec_dotfile.add_to_builder(
							builder,
							DotfileStatus::failed(format!(
								"Failed to apply content transformer `{}`: `{}`",
								transformer, err
							)),
						);

						return Ok(());
					}
				};
			}

			if !self.options.dry_run {
				if let Err(err) = std::fs::write(&exec_dotfile.deploy_path(), content.as_bytes()) {
					log::info!("{}: Failed to write content", exec_dotfile.path().display());

					exec_dotfile.add_to_builder(
						builder,
						DotfileStatus::failed(format!("Failed to write content: {}", err)),
					);

					return Ok(());
				}
			}
		}

		log::info!(
			"{}: Dotfile successfully deployed",
			exec_dotfile.path().display()
		);

		exec_dotfile.add_to_builder(builder, DotfileStatus::Success);

		Ok(())
	}
}

/// Resolves the deploy path for the given `dotfile`.
fn resolve_deployment_path(profile_target: &Path, dotfile: &Dotfile) -> PathBuf {
	dotfile
		.overwrite_target
		.as_deref()
		.unwrap_or(profile_target)
		.join(dotfile.rename.as_ref().unwrap_or(&dotfile.path))
}

/// Tries to resolve the source path for the given `dotfile`.
///
/// # Erros
///
/// An error is returned if the subsequent call to
/// [`std::path::Path::canonicalize`] fails.
fn resolve_source_path(source_path: &Path, dotfile: &Dotfile) -> std::io::Result<PathBuf> {
	source_path.join(&dotfile.path).canonicalize()
}
