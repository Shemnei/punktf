//! A [`Visit`](`crate::visit::Visitor`) implementation which deploys the items.

pub mod deployment;

use cfg_if::cfg_if;
use color_eyre::eyre::Context;

use crate::profile::{source::PunktfSource, MergeMode};
use crate::visit::*;

use crate::profile::transform::Transform as _;
use crate::profile::LayeredProfile;
use crate::visit::deploy::deployment::{Deployment, DeploymentBuilder, ItemStatus};
use std::borrow::Borrow;
use std::path::Path;

use crate::visit::{ResolvingVisitor, TemplateVisitor};

/// Represents the contents of a file as returned by [`safe_read`].
enum SafeRead {
	/// File was a normal text file.
	String(String),

	/// File was unable to be interpreted as a text file.
	Binary(Vec<u8>),
}

/// Reads the contents of a file, first trying to interpret them as a string and if that fails
/// returning the raw bytes.
fn safe_read<P: AsRef<Path>>(path: P) -> io::Result<SafeRead> {
	/// Inner function to reduce size of monomorphization.
	fn inner(path: &Path) -> io::Result<SafeRead> {
		match std::fs::read_to_string(path) {
			Ok(s) => Ok(SafeRead::String(s)),
			Err(err) if err.kind() == io::ErrorKind::InvalidData => {
				std::fs::read(path).map(SafeRead::Binary)
			}
			Err(err) => Err(err),
		}
	}

	inner(path.as_ref())
}

impl<'a> Item<'a> {
	/// Adds this item to the given
	/// [`DeploymentBuilder`](`crate::visit::deploy::deployment::DeploymentBuilder`).
	fn add_to_builder<S: Into<ItemStatus>>(&self, builder: &mut DeploymentBuilder, status: S) {
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

impl Symlink {
	/// Adds this item to the given
	/// [`DeploymentBuilder`](`crate::visit::deploy::deployment::DeploymentBuilder`).
	fn add_to_builder<S: Into<ItemStatus>>(&self, builder: &mut DeploymentBuilder, status: S) {
		builder.add_link(
			self.source_path.clone(),
			self.target_path.clone(),
			status.into(),
		);
	}
}

/// Marks the given item as successfully deployed.
macro_rules! success {
	($builder:expr, $item:expr) => {
		$item.add_to_builder($builder, ItemStatus::success());
	};
}

/// Marks the given item as skipped.
///
/// This will instantly return from the out function after reporting the skip.
macro_rules! skipped {
	($builder:expr, $item:expr, $reason:expr => $ret:expr ) => {
		$item.add_to_builder($builder, ItemStatus::skipped($reason));
		return Ok($ret);
	};
	($builder:expr, $item:expr, $reason:expr) => {
		$item.add_to_builder($builder, ItemStatus::skipped($reason));
		return Ok(());
	};
}

/// Marks the given item as failed.
///
/// This will instantly return from the out function after reporting the error.
macro_rules! failed {
	($builder:expr, $item:expr, $reason:expr => Err($ret:expr) ) => {
		$item.add_to_builder($builder, ItemStatus::failed($reason));
		return Err($ret);
	};
	($builder:expr, $item:expr, $reason:expr => $ret:expr ) => {
		$item.add_to_builder($builder, ItemStatus::failed($reason));
		return Ok($ret);
	};
	($builder:expr, $item:expr, $reason:expr) => {
		$item.add_to_builder($builder, ItemStatus::failed($reason));
		return Ok(());
	};
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
	/// Only hard errors will be returned as error, everything else will be
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

				skipped!(&mut self.builder, file, "Dotfile with higher priority is already deployed" => false);
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
						"{}: Overwriting existing dotfile",
						file.relative_source_path.display()
					)
				}
				MergeMode::Keep => {
					log::info!(
						"{}: Skipping existing dotfile",
						file.relative_source_path.display()
					);

					skipped!(&mut self.builder, file, format!("Dotfile already exists and merge mode is {:?}", MergeMode::Keep) => false);
				}
				MergeMode::Ask => {
					log::info!("{}: Asking for action", file.relative_source_path.display());

					let should_deploy = match (self.merge_ask_fn)(
						&file.source_path,
						file.target_path.borrow(),
					)
					.wrap_err("Error evaluating user response")
					{
						Ok(should_deploy) => should_deploy,
						Err(err) => {
							log::error!(
								"{}: Failed to execute ask function ({})",
								file.relative_source_path.display(),
								err
							);

							failed!(&mut self.builder, file, format!("Failed to execute merge ask function: {err}") => false);
						}
					};

					if !should_deploy {
						log::info!("{}: Merge was denied", file.relative_source_path.display());

						skipped!(&mut self.builder, file, "Dotfile already exists and merge ask was denied" => false);
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

						failed!(&mut self.builder, file, format!("Failed to create parent directory: {err}") => false);
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

					failed!(&mut self.builder, file, format!("Failed to apply content transformer `{transformer}`: `{err}`") => Err(err));
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
	/// Accepts a file item and tries to deploy it.
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
						"{}: Failed to copy file",
						file.relative_source_path.display()
					);

					failed!(&mut self.builder, file, format!("Failed to copy: {err}"));
				}
			}
		} else {
			let content = match safe_read(&file.source_path) {
				Ok(SafeRead::Binary(b)) => {
					log::info!(
						"[{}] Not evaluated as template - Binary data",
						file.relative_source_path.display()
					);

					b
				}
				Ok(SafeRead::String(s)) => {
					let Ok(content) = self.transform_content(profile, file, s) else {
						// Error is already recorded
						return Ok(());
					};

					content.into_bytes()
				}
				Err(err) => {
					log::info!(
						"{}: Failed to read file",
						file.relative_source_path.display()
					);

					failed!(&mut self.builder, file, format!("Failed to read: {err}"));
				}
			};

			if !self.options.dry_run {
				if let Err(err) = std::fs::write(&file.target_path, content) {
					log::info!(
						"{}: Failed to write content",
						file.relative_source_path.display()
					);

					failed!(
						&mut self.builder,
						file,
						format!("Failed to write content: {err}")
					);
				}
			}
		}

		log::info!(
			"{}: File successfully deployed",
			file.relative_source_path.display()
		);

		success!(&mut self.builder, file);

		Ok(())
	}

	/// Accepts a directory item and tries to deploy it.
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

				failed!(
					&mut self.builder,
					directory,
					format!("Failed to create directory: {err}")
				);
			} else {
				success!(&mut self.builder, directory);
			}
		} else {
			success!(&mut self.builder, directory);
		}

		log::info!(
			"{}: Directory successfully deployed",
			directory.relative_source_path.display()
		);

		Ok(())
	}

	/// Accepts a link item and tries to deploy it.
	fn accept_link(&mut self, _: &PunktfSource, _: &LayeredProfile, link: &Symlink) -> Result {
		log::info!("{}: Deploying symlink", link.source_path.display());

		// Log an warning if deploying of links is not supported for the
		// operating system.
		#[cfg(all(not(unix), not(windows)))]
		{
			log::warn!(
				"[{}]: Symlink operations are only supported for unix and windows systems",
				source_path.display()
			);
			skipped!(
				&mut self.builder,
				link,
				"Symlink operations are only supported on unix and windows systems"
			);
		}

		let source_path = &link.source_path;
		let target_path = &link.target_path;

		// Check that the source exists
		if !source_path.exists() {
			log::error!("[{}]: Links source does not exist", source_path.display());

			failed!(&mut self.builder, link, "Link source does not exist");
		}

		// Check that either the target does not exist or that i can be replaced
		if target_path.exists() {
			if link.replace {
				if !self.options.dry_run {
					// Verify that the target is a symlink
					let target_metadata = match target_path.symlink_metadata() {
						Ok(m) => m,
						Err(err) => {
							log::error!("[{}]: Failed to read metadata", source_path.display());

							failed!(
								&mut self.builder,
								link,
								format!("Failed get link target metadata: {err}")
							);
						}
					};

					if target_metadata.is_symlink() {
						// Get metadata of symlink target
						let res = if let Ok(target_metadata) = target_path.metadata() {
							if target_metadata.is_dir() {
								std::fs::remove_dir(target_path)
							} else {
								std::fs::remove_file(target_path)
							}
						} else {
							std::fs::remove_file(target_path)
								.or_else(|_| std::fs::remove_dir(target_path))
						};

						if let Err(err) = res {
							log::error!(
								"[{}]: Failed to remove old link at target",
								source_path.display()
							);

							failed!(
								&mut self.builder,
								link,
								format!("Failed to remove old link target: {err}")
							);
						} else {
							log::info!(
								"[{}]: Removed old link target at {}",
								source_path.display(),
								target_path.display()
							);
						}
					} else {
						log::error!(
							"[{}]: Target already exists and is no link",
							source_path.display()
						);

						failed!(&mut self.builder, link, "Not allowed to replace target");
					}
				}
			} else {
				log::error!(
					"[{}]: Target already exists and is not allowed to be replaced",
					source_path.display()
				);

				skipped!(&mut self.builder, link, "Link target does already exist");
			}
		}

		if !self.options.dry_run {
			cfg_if! {
				if #[cfg(unix)] {
					if let Err(err) = std::os::unix::fs::symlink(source_path, target_path) {
						log::error!("[{}]: Failed to create link", source_path.display());

						failed!(&mut self.builder, link, format!("Failed create link: {err}"));
					};
				} else if #[cfg(windows)] {
					let metadata = match source_path.symlink_metadata() {
						Ok(m) => m,
						Err(err) => {
							log::error!("[{}]: Failed to read metadata", source_path.display());

							failed!(&mut self.builder, link, format!("Failed get link source metadata: {err}"));
						}
					};

					if metadata.is_dir() {
						if let Err(err) = std::os::windows::fs::symlink_dir(source_path, target_path) {
							log::error!("[{}]: Failed to create directory link", source_path.display());

							failed!(&mut self.builder, link, format!("Failed create directory link: {err}"));
						};
					} else if metadata.is_file() {
						if let Err(err) = std::os::windows::fs::symlink_file(source_path, target_path) {
							log::error!("[{}]: Failed to create file link", source_path.display());

							failed!(&mut self.builder, link, format!("Failed create file link: {err}"));
						};
					} else {
						log::error!("[{}]: Invalid link source type", source_path.display());

						failed!(&mut self.builder, link, "Invalid type of link source");
					}
				} else {
					log::warn!("[{}]: Link operations are only supported for unix and windows systems", source_path.display());

					skipped!(&mut self.builder, link, "Link operations are only supported on unix and windows systems");
				}
			}
		}

		success!(&mut self.builder, link);

		Ok(())
	}

	/// Accepts a rejected item and reports it.
	fn accept_rejected<'a>(
		&mut self,
		_: &PunktfSource,
		_: &LayeredProfile,
		rejected: &Rejected<'a>,
	) -> Result {
		log::info!(
			"[{}]: Rejected - {}",
			rejected.relative_source_path.display(),
			rejected.reason
		);

		skipped!(&mut self.builder, rejected, rejected.reason.clone());
	}

	/// Accepts a errored item and reports it.
	fn accept_errored<'a>(
		&mut self,
		_: &PunktfSource,
		_: &LayeredProfile,
		errored: &Errored<'a>,
	) -> Result {
		log::error!(
			"[{}]: Failed - {}",
			errored.relative_source_path.display(),
			errored
		);

		failed!(&mut self.builder, errored, errored.to_string());
	}
}

impl<F> TemplateVisitor for Deployer<F>
where
	F: Fn(&Path, &Path) -> color_eyre::Result<bool>,
{
	/// Accepts a file template item and tries to deploy it.
	///
	/// Before the deployment the template is parsed and resolved.
	fn accept_template<'a>(
		&mut self,
		_: &PunktfSource,
		profile: &LayeredProfile,
		file: &File<'a>,
		// Returns a function to resolve the content to make the resolving lazy
		// for upstream visitors.
		resolve_content: impl FnOnce(&str) -> color_eyre::Result<String>,
	) -> Result {
		log::info!(
			"{}: Deploying template",
			file.relative_source_path.display()
		);

		let cont = self.pre_deploy_checks(file)?;

		if !cont {
			return Ok(());
		}

		let content = match safe_read(&file.source_path) {
			Ok(SafeRead::Binary(b)) => {
				log::info!(
					"[{}] Not evaluated as template - Binary data",
					file.relative_source_path.display()
				);

				b
			}
			Ok(SafeRead::String(s)) => {
				let content = match resolve_content(&s) {
					Ok(content) => content,
					Err(err) => {
						log::info!(
							"{}: Failed to resolve template",
							file.relative_source_path.display()
						);

						failed!(
							&mut self.builder,
							file,
							format!("Failed to resolve template: {err}")
						);
					}
				};

				let Ok(content) = self.transform_content(profile, file, content) else {
					// Error is already recorded
					return Ok(());
				};

				content.into_bytes()
			}
			Err(err) => {
				log::info!(
					"{}: Failed to read file",
					file.relative_source_path.display()
				);

				failed!(&mut self.builder, file, format!("Failed to read: {err}"));
			}
		};

		if !self.options.dry_run {
			if let Err(err) = std::fs::write(&file.target_path, content) {
				log::info!(
					"{}: Failed to write content",
					file.relative_source_path.display()
				);

				failed!(
					&mut self.builder,
					file,
					format!("Failed to write content: {err}")
				);
			}
		}

		log::info!(
			"{}: Template successfully deployed",
			file.relative_source_path.display()
		);

		success!(&mut self.builder, file);

		Ok(())
	}
}
