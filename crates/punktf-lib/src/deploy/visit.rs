use color_eyre::eyre::Context;

use crate::template::Template;
use crate::{profile::visit::*, template::source::Source};
use crate::{MergeMode, PunktfSource};

use crate::profile::LayeredProfile;
use crate::transform::Transform as _;
use std::borrow::Borrow;
use std::path::Path;

use super::deployment::{Deployment, DeploymentBuilder};
use super::dotfile::DotfileStatus;
use super::executor::ExecutorOptions;

pub trait TemplateVisitor: Visitor {
	fn accept_template<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		file: &File<'a>,
		// Returns a function to resolve the content to make the resolving lazy
		// for upstream visitors.
		resolve_content: impl FnOnce(&str) -> color_eyre::Result<String>,
	) -> Result;
}

#[derive(Debug)]
pub struct ResolvingVisitor<V> {
	visitor: V,
}

impl<V> ResolvingVisitor<V>
where
	V: TemplateVisitor,
{
	pub fn new(visitor: V) -> Self {
		Self { visitor }
	}

	pub fn into_inner(self) -> V {
		self.visitor
	}
}

impl<V: TemplateVisitor> Visitor for ResolvingVisitor<V> {
	fn accept_file<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		file: &File<'a>,
	) -> Result {
		println!("F: {:#?}", file);

		if file.dotfile().is_template() {
			let resolve_fn = |content: &str| {
				let source = Source::file(&file.source_path, content);
				let template = Template::parse(source)
					.with_context(|| format!("File: {}", file.source_path.display()))?;

				template
					.resolve(Some(profile.variables()), file.dotfile().variables.as_ref())
					.with_context(|| format!("File: {}", file.source_path.display()))
			};

			self.visitor
				.accept_template(source, profile, file, resolve_fn)
		} else {
			self.visitor.accept_file(source, profile, file)
		}
	}

	fn accept_directory<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		directory: &Directory<'a>,
	) -> Result {
		println!("D: {:#?}", directory);

		self.visitor.accept_directory(source, profile, directory)
	}

	fn accept_link(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		symlink: &Symlink,
	) -> Result {
		self.visitor.accept_link(source, profile, symlink)
	}

	fn accept_rejected<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		rejected: &Rejected<'a>,
	) -> Result {
		self.visitor.accept_rejected(source, profile, rejected)
	}

	fn accept_errored<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		errored: &Errored<'a>,
	) -> Result {
		self.visitor.accept_errored(source, profile, errored)
	}
}

impl<'a> File<'a> {
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

impl<'a> Directory<'a> {
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

impl<'a> Rejected<'a> {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeployingVisitor<F> {
	options: ExecutorOptions,
	merge_ask_fn: F,
	builder: DeploymentBuilder,
}

impl<F> DeployingVisitor<F>
where
	F: Fn(&Path, &Path) -> color_eyre::Result<bool>,
{
	pub fn new(options: ExecutorOptions, merge_ask_fn: F) -> Self {
		Self {
			options,
			merge_ask_fn,
			builder: DeploymentBuilder::default(),
		}
	}

	pub fn into_deployment(self) -> Deployment {
		self.builder.finish()
	}

	fn pre_deploy_checks(
		&mut self,
		source: &PunktfSource,
		file: &File<'_>,
	) -> color_eyre::Result<bool> {
		let relative_source_path = file
			.source_path
			.strip_prefix(&source.dotfiles)
			.with_context(|| {
				format!(
					"{}: Failed to resolve relative source path",
					file.source_path.display(),
				)
			})?;

		let other_priority = self.builder.get_priority(&file.target_path).unwrap_or(None);

		match (file.dotfile().priority.as_ref(), other_priority.as_ref()) {
			(Some(a), Some(b)) if b > a => {
				log::info!(
					"{}: Dotfile with higher priority is already deployed at {}",
					relative_source_path.display(),
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
				relative_source_path.display(),
				file.target_path.display()
			);

			match file.dotfile().merge.unwrap_or_default() {
				MergeMode::Overwrite => {
					log::info!(
						"{}: Overwritting existing dotfile",
						relative_source_path.display()
					)
				}
				MergeMode::Keep => {
					log::info!(
						"{}: Skipping existing dotfile",
						relative_source_path.display()
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
					log::info!("{}: Asking for action", relative_source_path.display());

					let should_deploy =
						match (self.merge_ask_fn)(&file.source_path, file.target_path.borrow())
							.wrap_err("Error evaluating user response")
						{
							Ok(should_deploy) => should_deploy,
							Err(err) => {
								log::error!(
									"{}: Failed to execute ask function ({})",
									relative_source_path.display(),
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
						log::info!("{}: Merge was denied", relative_source_path.display());

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
							relative_source_path.display(),
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

	fn transform_content(
		&mut self,
		profile: &LayeredProfile,
		file: &File<'_>,
		relative_source_path: &Path,
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
						relative_source_path.display(),
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

impl<F> Visitor for DeployingVisitor<F>
where
	F: Fn(&Path, &Path) -> color_eyre::Result<bool>,
{
	fn accept_file<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		file: &File<'a>,
	) -> Result {
		let cont = self.pre_deploy_checks(source, file)?;

		if !cont {
			return Ok(());
		}

		let relative_source_path = file
			.source_path
			.strip_prefix(&source.dotfiles)
			.with_context(|| {
				format!(
					"{}: Failed to resolve relative source path",
					file.source_path.display(),
				)
			})?;

		// Fast path
		if profile.transformers_len() == 0 && file.dotfile().transformers.is_empty() {
			// File is no template and no transformers are specified. This means
			// we can take the fast path of just copying via the filesystem.

			// Allowed for readability
			#[allow(clippy::collapsible_else_if)]
			if !self.options.dry_run {
				if let Err(err) = std::fs::copy(&file.source_path, &file.target_path) {
					log::info!("{}: Failed to copy dotfile", relative_source_path.display());

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
					log::info!("{}: Failed to read dotfile", relative_source_path.display());

					file.add_to_builder(
						&mut self.builder,
						DotfileStatus::failed(format!("Failed to read: {}", err)),
					);

					return Ok(());
				}
			};

			let Ok(content) = self.transform_content(profile, file, relative_source_path, content) else {
				// Error is already recorded
				return Ok(());
			};

			if !self.options.dry_run {
				if let Err(err) = std::fs::write(&file.target_path, content.as_bytes()) {
					log::info!(
						"{}: Failed to write content",
						relative_source_path.display()
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
			relative_source_path.display()
		);

		file.add_to_builder(&mut self.builder, DotfileStatus::Success);

		Ok(())
	}

	fn accept_directory<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		directory: &Directory<'a>,
	) -> Result {
		let relative_source_path = directory
			.source_path
			.strip_prefix(&source.dotfiles)
			.with_context(|| {
				format!(
					"{}: Failed to resolve relative source path",
					directory.source_path.display(),
				)
			})?;

		let target_path = directory
			.target_path
			.canonicalize()
			.unwrap_or(directory.target_path.clone());

		if !self.options.dry_run {
			if let Err(err) = std::fs::create_dir_all(target_path) {
				log::error!(
					"{}: Failed to create directory ({})",
					relative_source_path.display(),
					err
				);

				directory.add_to_builder(
					&mut self.builder,
					DotfileStatus::failed(format!("Failed to create directory: {}", err)),
				);
			}
		}

		Ok(())
	}

	fn accept_link(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		symlink: &Symlink,
	) -> Result {
		todo!()
	}

	fn accept_rejected<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		rejected: &Rejected<'a>,
	) -> Result {
		rejected.add_to_builder(
			&mut self.builder,
			DotfileStatus::skipped(rejected.reason.to_owned()),
		);

		Ok(())
	}

	fn accept_errored<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		errored: &Errored<'a>,
	) -> Result {
		todo!("ERRORED: {:#?}", errored);
	}
}

impl<F> TemplateVisitor for DeployingVisitor<F>
where
	F: Fn(&Path, &Path) -> color_eyre::Result<bool>,
{
	fn accept_template<'a>(
		&mut self,
		source: &PunktfSource,
		profile: &LayeredProfile,
		file: &File<'a>,
		// Returns a function to resolve the content to make the resolving lazy
		// for upstream visitors.
		resolve_content: impl FnOnce(&str) -> color_eyre::Result<String>,
	) -> Result {
		let cont = self.pre_deploy_checks(source, file)?;

		if !cont {
			return Ok(());
		}

		let relative_source_path = file
			.source_path
			.strip_prefix(&source.dotfiles)
			.with_context(|| {
				format!(
					"{}: Failed to resolve relative source path",
					file.source_path.display(),
				)
			})?;

		let content = match std::fs::read_to_string(&file.source_path) {
			Ok(content) => content,
			Err(err) => {
				log::info!("{}: Failed read dotfile", relative_source_path.display());

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
					relative_source_path.display()
				);

				file.add_to_builder(
					&mut self.builder,
					DotfileStatus::failed(format!("Failed to resolve template: {}", err)),
				);

				return Ok(());
			}
		};

		let Ok(content) = self.transform_content(profile, file, relative_source_path, content) else {
				// Error is already recorded
				return Ok(());
			};

		if !self.options.dry_run {
			if let Err(err) = std::fs::write(&file.target_path, content.as_bytes()) {
				log::info!(
					"{}: Failed to write content",
					relative_source_path.display()
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
			relative_source_path.display()
		);

		file.add_to_builder(&mut self.builder, DotfileStatus::Success);

		Ok(())
	}
}