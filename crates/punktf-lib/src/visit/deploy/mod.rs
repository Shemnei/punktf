//! A [`Visit`](`crate::visit::Visitor`) implementation which deploys the items.

pub mod deployment;

use cfg_if::cfg_if;
use color_eyre::eyre::Context;

use crate::profile::{source::PunktfSource, MergeMode};
use crate::visit::*;

use crate::profile::transform::Transform as _;
use crate::profile::LayeredProfile;
use crate::visit::deploy::deployment::{Deployment, DeploymentBuilder, DotfileStatus};
use std::borrow::Borrow;
use std::path::Path;

use crate::visit::{ResolvingVisitor, TemplateVisitor};

impl<'a> Item<'a> {
	/// Adds this item to the given
	/// [`DeploymentBuilder`](`crate::visit::deploy::deployment::DeploymentBuilder`).
	fn add_to_builder<S: Into<DotfileStatus>>(&self, builder: &mut DeploymentBuilder, status: S) {
		let status = status.into();

		let resolved_target_path = self
			.target_path
			.canonicalize()
			.unwrap_or_else(|_| self.target_path.clone());

		match &self.kind {
			Kind::Root(dotfile) => {
				builder.add_dotfile(resolved_target_path, (*dotfile).clone(), status)
			}
			Kind::Child {
				root_target_path, ..
			} => {
				let resolved_root_target_path = root_target_path
					.canonicalize()
					.unwrap_or_else(|_| root_target_path.clone());

				builder.add_child(resolved_target_path, resolved_root_target_path, status)
			}
		};
	}
}

/// Configuration options for the [`Deployer`].
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeployOptions {
	/// If this flag is set, it will prevent any write operations from occurring
	/// during the deployment.
	///
	/// This includes write, copy and directory creation operations.
	pub dry_run: bool,
}

/// Responsible for deploying a [profile](`crate::profile::Profile`).
///
/// This includes checking for merge conflicts, resolving children of a
/// directory dotfile, parsing and resolving of templates and the actual
/// writing of the dotfile to the target destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deployer<F> {
	/// Configuration options
	options: DeployOptions,

	/// This function gets called when a dotfile at the target destination
	/// already exists and the merge mode is
	/// [MergeMode::Ask](`crate::profile::MergeMode::Ask`).
	///
	/// The arguments for the function are (dotfile_source_path, dotfile_target_path).
	merge_ask_fn: F,

	/// Builder for the deployment.
	///
	/// This holds information about each item which was processed,
	/// keeps track of the time and also stores a overall status of the deployment.
	builder: DeploymentBuilder,
}

impl<F> Deployer<F>
where
	F: Fn(&Path, &Path) -> color_eyre::Result<bool>,
{
	/// Creates a new instance.
	pub fn new(options: DeployOptions, merge_ask_fn: F) -> Self {
		Self {
			options,
			merge_ask_fn,
			builder: DeploymentBuilder::default(),
		}
	}

	/// Retrieves the finished deployment from this instance.
	pub fn into_deployment(self) -> Deployment {
		self.builder.finish()
	}

	/// Tries to deploy the given `profile`.
	///
	/// # Errors
	///
	/// Only hard errors will be returned as error, everthing else will be
	/// recorded in the [Deployment](`crate::visit::deploy::deployment::Deployment`)
	/// on a dotfile level.
	pub fn deploy(self, source: &PunktfSource, profile: &mut LayeredProfile) -> Deployment {
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

		for hook in profile.pre_hooks() {
			log::info!("Executing pre-hook: {}", hook.command());
			// No files are deployed yet, meaning if an error during hook
			// execution occurs it will return with an error instead of just
			// logging it.

			if let Err(err) = hook
				.execute(source.profiles())
				.wrap_err("Failed to execute pre-hook")
			{
				log::error!("Failed to execute pre-hook ({})", err);
				return self.builder.failed(err.to_string());
			};
		}

		let mut resolver = ResolvingVisitor(self);
		let walker = Walker::new(profile);
		if let Err(err) = walker.walk(source, &mut resolver) {
			return resolver.into_inner().builder.failed(err.to_string());
		}

		let this = resolver.into_inner();

		for hook in profile.post_hooks() {
			log::info!("Executing post-hook: {}", hook.command());
			if let Err(err) = hook.execute(source.profiles()) {
				log::error!("Failed to execute post-hook ({})", err);
				return this.builder.failed(err.to_string());
			}
		}

		this.into_deployment()
	}

	/// Checks common things for a given file item before deploying it.
	///
	/// The returned boolean indicates if the deployment of the file should
	/// continue.
	fn pre_deploy_checks(&mut self, file: &File<'_>) -> color_eyre::Result<bool> {
		let other_priority = self.builder.get_priority(&file.target_path);

		match (file.dotfile().priority.as_ref(), other_priority) {
			(Some(a), Some(b)) if b > a => {
				log::info!(
					"{}: Dotfile with higher priority is already deployed at {}",
					file.relative_source_path.display(),
					file.target_path.display()
				);

				file.add_to_builder(
					&mut self.builder,
					DotfileStatus::skipped("Dotfile with higher priority is already deployed"),
				);

				return Ok(false);
			}
			(_, _) => {}
		};

		if file.target_path.exists() {
			// No previously deployed dotfile at `deploy_path`. Check for merge.

			log::debug!(
				"{}: Dotfile already exists at {}",
				file.relative_source_path.display(),
				file.target_path.display()
			);

			match file.dotfile().merge.unwrap_or_default() {
				MergeMode::Overwrite => {
					log::info!(
						"{}: Overwritting existing dotfile",
						file.relative_source_path.display()
					)
				}
				MergeMode::Keep => {
					log::info!(
						"{}: Skipping existing dotfile",
						file.relative_source_path.display()
					);

					file.add_to_builder(
						&mut self.builder,
						DotfileStatus::skipped(format!(
							"Dotfile already exists and merge mode is {:?}",
							MergeMode::Keep,
						)),
					);

					return Ok(false);
				}
				MergeMode::Ask => {
					log::info!("{}: Asking for action", file.relative_source_path.display());

					let should_deploy =
						match (self.merge_ask_fn)(&file.source_path, file.target_path.borrow())
							.wrap_err("Error evaluating user response")
						{
							Ok(should_deploy) => should_deploy,
							Err(err) => {
								log::error!(
									"{}: Failed to execute ask function ({})",
									file.relative_source_path.display(),
									err
								);

								file.add_to_builder(
									&mut self.builder,
									DotfileStatus::failed(format!(
										"Failed to execute merge ask function: {}",
										err
									)),
								);

								return Ok(false);
							}
						};

					if !should_deploy {
						log::info!("{}: Merge was denied", file.relative_source_path.display());

						file.add_to_builder(
							&mut self.builder,
							DotfileStatus::skipped(
								"Dotfile already exists and merge ask was denied",
							),
						);

						return Ok(false);
					}
				}
			}
		}

		if let Some(parent) = file.target_path.parent() {
			if !self.options.dry_run {
				match std::fs::create_dir_all(parent) {
					Ok(_) => {}
					Err(err) => {
						log::error!(
							"{}: Failed to create directory ({})",
							file.relative_source_path.display(),
							err
						);

						file.add_to_builder(
							&mut self.builder,
							DotfileStatus::failed(format!(
								"Failed to create parent directory: {}",
								err
							)),
						);

						return Ok(false);
					}
				}
			}
		}

		Ok(true)
	}

	/// Applies any relevant [`Transform`](`crate::profile::transform::Transform`)
	/// for the given file.
	fn transform_content(
		&mut self,
		profile: &LayeredProfile,
		file: &File<'_>,
		content: String,
	) -> color_eyre::Result<String> {
		let mut content = content;

		// Copy so we exec_dotfile is not referenced by this in case an error occurs.
		let exec_transformers: Vec<_> = file.dotfile().transformers.to_vec();

		// Apply transformers.
		// Order:
		//   - Transformers which are specified in the profile root
		//   - Transformers which are specified on a specific dotfile of a profile
		for transformer in profile.transformers().chain(exec_transformers.iter()) {
			content = match transformer.transform(content) {
				Ok(content) => content,
				Err(err) => {
					log::info!(
						"{}: Failed to apply content transformer `{}`: `{}`",
						file.relative_source_path.display(),
						transformer,
						err
					);

					file.add_to_builder(
						&mut self.builder,
						DotfileStatus::failed(format!(
							"Failed to apply content transformer `{}`: `{}`",
							transformer, err
						)),
					);

					return Err(err);
				}
			};
		}

		Ok(content)
	}
}

impl<F> Visitor for Deployer<F>
where
	F: Fn(&Path, &Path) -> color_eyre::Result<bool>,
{
	fn accept_file<'a>(
		&mut self,
		_: &PunktfSource,
		profile: &LayeredProfile,
		file: &File<'a>,
	) -> Result {
		log::info!("{}: Deploying file", file.relative_source_path.display());

		let cont = self.pre_deploy_checks(file)?;

		if !cont {
			return Ok(());
		}

		// Fast path
		if profile.transformers_len() == 0 && file.dotfile().transformers.is_empty() {
			// File is no template and no transformers are specified. This means
			// we can take the fast path of just copying via the filesystem.

			// Allowed for readability
			#[allow(clippy::collapsible_else_if)]
			if !self.options.dry_run {
				if let Err(err) = std::fs::copy(&file.source_path, &file.target_path) {
					log::info!(
						"{}: Failed to copy dotfile",
						file.relative_source_path.display()
					);

					file.add_to_builder(
						&mut self.builder,
						DotfileStatus::failed(format!("Failed to copy: {}", err)),
					);

					return Ok(());
				}
			}
		} else {
			let content = match std::fs::read_to_string(&file.source_path) {
				Ok(content) => content,
				Err(err) => {
					log::info!(
						"{}: Failed to read dotfile",
						file.relative_source_path.display()
					);

					file.add_to_builder(
						&mut self.builder,
						DotfileStatus::failed(format!("Failed to read: {}", err)),
					);

					return Ok(());
				}
			};

			let Ok(content) = self.transform_content(profile, file, content) else {
				// Error is already recorded
				return Ok(());
			};

			if !self.options.dry_run {
				if let Err(err) = std::fs::write(&file.target_path, content.as_bytes()) {
					log::info!(
						"{}: Failed to write content",
						file.relative_source_path.display()
					);

					file.add_to_builder(
						&mut self.builder,
						DotfileStatus::failed(format!("Failed to write content: {}", err)),
					);

					return Ok(());
				}
			}
		}

		log::info!(
			"{}: Dotfile successfully deployed",
			file.relative_source_path.display()
		);

		file.add_to_builder(&mut self.builder, DotfileStatus::Success);

		Ok(())
	}

	fn accept_directory<'a>(
		&mut self,
		_: &PunktfSource,
		_: &LayeredProfile,
		directory: &Directory<'a>,
	) -> Result {
		log::info!(
			"{}: Deploying directory",
			directory.relative_source_path.display()
		);

		if !self.options.dry_run {
			if let Err(err) = std::fs::create_dir_all(&directory.target_path) {
				log::error!(
					"{}: Failed to create directory ({})",
					directory.relative_source_path.display(),
					err
				);

				directory.add_to_builder(
					&mut self.builder,
					DotfileStatus::failed(format!("Failed to create directory: {}", err)),
				);
			} else {
				directory.add_to_builder(&mut self.builder, DotfileStatus::success());
			}
		} else {
			directory.add_to_builder(&mut self.builder, DotfileStatus::success());
		}

		Ok(())
	}

	fn accept_link(&mut self, _: &PunktfSource, _: &LayeredProfile, link: &Symlink) -> Result {
		let source_path = &link.source_path;
		let target_path = &link.target_path;

		if !source_path.exists() {
			self.builder.add_link(
				source_path.clone(),
				target_path.clone(),
				DotfileStatus::failed("Link source does not exist"),
			);

			return Ok(());
		}

		if target_path.exists() {
			self.builder.add_link(
				source_path.clone(),
				target_path.clone(),
				DotfileStatus::skipped("Link target does already exist"),
			);

			return Ok(());
		}

		if !self.options.dry_run {
			cfg_if! {
				if #[cfg(unix)] {
					if let Err(err) = std::os::unix::fs::symlink(source_path, target_path) {
						self.builder.add_link(
							source_path.clone(),
							target_path.clone(),
							DotfileStatus::failed(format!("Failed create symlink: {}", err)),
						);
					};
				} else if #[cfg(windows)] {
					let metadata = match source_path.symlink_metadata() {
						Ok(m) => m,
						Err(err) => {
							self.builder.add_link(
								source_path.clone(),
								target_path.clone(),
								DotfileStatus::failed(format!("Failed get link source metadata: {}", err)),
							);

							return Ok(());
						}
					};

					if metadata.is_dir() {
						if let Err(err) =
							std::os::windows::fs::symlink_dir(source_path, target_path)
						{
							self.builder.add_link(
								source_path.clone(),
								target_path.clone(),
								DotfileStatus::failed(format!("Failed create directory symlink: {}", err)),
							);
						};
					} else if metadata.is_file() {
						if let Err(err) =
							std::os::windows::fs::symlink_file(source_path, target_path)
						{
							self.builder.add_link(
								source_path.clone(),
								target_path.clone(),
								DotfileStatus::failed(format!("Failed create file symlink: {}", err)),
							);
						};
					} else {
						self.builder.add_link(
							source_path.clone(),
							target_path.clone(),
							DotfileStatus::failed("Invalid type of symlink source"),
						);
					}
				} else {
					self.builder.add_link(
						source_path.clone(),
						target_path.clone(),
						DotfileStatus::skipped("Symlink operations are only supported on unix and windows systems"),
					);
				}
			}
		} else {
			self.builder.add_link(
				source_path.clone(),
				target_path.clone(),
				DotfileStatus::success(),
			);
		}

		Ok(())
	}

	fn accept_rejected<'a>(
		&mut self,
		_: &PunktfSource,
		_: &LayeredProfile,
		rejected: &Rejected<'a>,
	) -> Result {
		rejected.add_to_builder(
			&mut self.builder,
			DotfileStatus::skipped(rejected.reason.clone()),
		);

		Ok(())
	}

	fn accept_errored<'a>(
		&mut self,
		_: &PunktfSource,
		_: &LayeredProfile,
		errored: &Errored<'a>,
	) -> Result {
		errored.add_to_builder(
			&mut self.builder,
			DotfileStatus::failed(format!("{}", errored)),
		);

		Ok(())
	}
}

impl<F> TemplateVisitor for Deployer<F>
where
	F: Fn(&Path, &Path) -> color_eyre::Result<bool>,
{
	fn accept_template<'a>(
		&mut self,
		_: &PunktfSource,
		profile: &LayeredProfile,
		file: &File<'a>,
		// Returns a function to resolve the content to make the resolving lazy
		// for upstream visitors.
		resolve_content: impl FnOnce(&str) -> color_eyre::Result<String>,
	) -> Result {
		let cont = self.pre_deploy_checks(file)?;

		if !cont {
			return Ok(());
		}

		let content = match std::fs::read_to_string(&file.source_path) {
			Ok(content) => content,
			Err(err) => {
				log::info!(
					"{}: Failed read dotfile",
					file.relative_source_path.display()
				);

				file.add_to_builder(
					&mut self.builder,
					DotfileStatus::failed(format!("Failed to read: {}", err)),
				);

				return Ok(());
			}
		};

		let content = match resolve_content(&content) {
			Ok(content) => content,
			Err(err) => {
				log::info!(
					"{}: Failed to resolve template",
					file.relative_source_path.display()
				);

				file.add_to_builder(
					&mut self.builder,
					DotfileStatus::failed(format!("Failed to resolve template: {}", err)),
				);

				return Ok(());
			}
		};

		let Ok(content) = self.transform_content(profile, file, content) else {
				// Error is already recorded
				return Ok(());
			};

		if !self.options.dry_run {
			if let Err(err) = std::fs::write(&file.target_path, content.as_bytes()) {
				log::info!(
					"{}: Failed to write content",
					file.relative_source_path.display()
				);

				file.add_to_builder(
					&mut self.builder,
					DotfileStatus::failed(format!("Failed to write content: {}", err)),
				);

				return Ok(());
			}
		}

		log::info!(
			"{}: Dotfile successfully deployed",
			file.relative_source_path.display()
		);

		file.add_to_builder(&mut self.builder, DotfileStatus::Success);

		Ok(())
	}
}
