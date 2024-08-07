//! punktf - A cross-platform multi-target dotfiles manager
//!
//! ## Yet another dotfile manager?!
//!
//! Well, yes, but hear me out: This project was driven by the personal need of having to manage several dotfiles for different machines/targets. You want the same experience everywhere: On your Windows workstation along with an Ubuntu WSL instance, your Debian server and your private Arch installation. This tool fixes that problem while being cross-platform and blazingly fast. You won't need multiple sets of dotfile configurations ever again!
//!
//! Features:
//!
//! - Compile and deploy your dotfiles with one command across different platforms
//! - Use handlebar-like instructions to insert variables and compile sections conditionally
//! - Define pre- and post-hooks to customize the behavior with your own commands
//! - Create multiple profiles for different targets
//! - Works on Windows and Linux
//!
//! ## Usage
//!
//! ### Commands
//!
//! To deploy a profile, use the `deploy` subcommand:
//!
//! ```sh
//! # deploy 'windows' profile
//! punktf deploy windows
//!
//! # deploy (custom source folder)
//! punktf --source /home/demo/mydotfiles deploy windows
//! ```
//!
//! Adding the `-h`/`--help` flag to a given subcommand, will print usage instructions.
//!
//! ### Source Folder
//!
//! The punktf source folder, is the folder containing the dotfiles and punktf profiles. We recommend setting the `PUNKTF_SOURCE` environment variable, so that the dotfiles can be compiled using `punktf deploy <profile>`.
//!
//! punktf searches for the source folder in the following order:
//!
//! 1. CLI argument given with `-s`/`--source`
//! 2. Environment variable `PUNKTF_SOURCE`
//! 3. Current working directory of the shell
//!
//! The source folder should contain two sub-folders:
//!
//! * `profiles\`: Contains the punktf profile definitions (`.yaml` or `.json`)
//! * `dotfiles\`: Contains folders and the actual dotfiles
//!
//! Example punktf source folder structure:
//!
//! ```ls
//! + profiles
//!     + windows.yaml
//!     + base.yaml
//!     + arch.json
//! + dotfiles
//!     + .gitconfig
//!     + init.vim.win
//!     + base
//!         + demo.txt
//!     + linux
//!         + .bashrc
//!     + windows
//!         + alacritty.yml
//! ```
//!
//! ### Target
//!
//! Determines where `punktf` will deploy files too.
//! It can be set with:
//!
//! 1. Variable `target` in the punktf profile file
//! 2. Environment variable `PUNKTF_TARGET`
//!
//! ### Profiles
//!
//! Profiles define which dotfiles should be used. They can be a `.json` or `.yaml` file.
//!
//! Example punktf profile:
//!
//! ```yaml
//! variables:
//!   OS: "windows"
//!
//! target: "C:\\Users\\Demo"
//!
//! dotfiles:
//!   - path: "base"
//!   - path: "windows/alacritty.yml"
//!     target:
//!         Path: "C:\\Users\\Demo\\AppData\\Local\\alacritty.yml"
//!     merge: Ask
//! ```
//!
//! All properties are explained [in the wiki](https://github.com/Shemnei/punktf/wiki/Profiles).
//!
//! ## Templates
//!
//! Please refer to the [wiki](https://github.com/Shemnei/punktf/wiki/Templating) for the templating syntax.
//!
//! ## Dotfile Repositories using punktf
//!
//! - [michidk/dotfiles](https://gitlab.com/michidk/dotfiles)

#![allow(rustdoc::private_intra_doc_links)]
#![deny(
	dead_code,
	deprecated_in_future,
	exported_private_dependencies,
	future_incompatible,
	missing_copy_implementations,
	rustdoc::missing_crate_level_docs,
	rustdoc::broken_intra_doc_links,
	missing_docs,
	clippy::missing_docs_in_private_items,
	missing_debug_implementations,
	rust_2018_compatibility,
	rust_2018_idioms,
	trivial_casts,
	trivial_numeric_casts,
	unsafe_code,
	unstable_features,
	unused_import_braces,
	unused_qualifications,

	// clippy attributes
	clippy::missing_const_for_fn,
	clippy::redundant_pub_crate,
	clippy::use_self
)]
#![cfg_attr(docsrs, feature(doc_cfg), feature(doc_alias))]

mod diff;
mod opt;
mod util;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use clap::{CommandFactory, Parser};
use color_eyre::eyre::eyre;
use color_eyre::Result;
use opt::Command;
use punktf_lib::profile::dotfile::Dotfile;
use punktf_lib::profile::source::PunktfSource;
use punktf_lib::profile::{resolve_profile, LayeredProfile, Profile};
use punktf_lib::template::source::Source;
use punktf_lib::template::Template;
use punktf_lib::visit::deploy::{deployment::Deployment, *};
use punktf_lib::visit::diff::Diff;

/// Name of this binary.
const BINARY_NAME: &str = env!("CARGO_BIN_NAME");

/// Name of the environment variable which defines the default source path for
/// `punktf`.
pub const PUNKTF_SOURCE_ENVVAR: &str = "PUNKTF_SOURCE";

/// Name of the environment variable which defines the default target path for
/// `punktf`.
pub const PUNKTF_TARGET_ENVVAR: &str = "PUNKTF_TARGET";

/// Name of the environment variable which defines the default profile for
/// `punktf`.
pub const PUNKTF_PROFILE_ENVVAR: &str = "PUNKTF_PROFILE";

/// Entry point for `punktf`.
fn main() -> Result<()> {
	color_eyre::install()?;

	let opts = opt::Opts::parse();

	let log_level = if opts.shared.quite {
		log::Level::Error
	} else {
		match opts.shared.verbose {
			// Default if no value for `verbose` is given
			0 => log::Level::Warn,
			1 => log::Level::Info,
			2 => log::Level::Debug,
			_ => log::Level::Trace,
		}
	};

	env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level.as_str()))
		.init();

	log::debug!("Parsed Opts:\n{:#?}", opts);

	handle_command(opts.command)
}

/// Gets the parsed command line arguments and evaluates them.
fn handle_command(command: Command) -> Result<()> {
	match command {
		Command::Deploy(c) => handle_command_deploy(c),
		Command::Render(c) => handle_command_render(c),
		Command::Verify(c) => handle_command_verify(c),
		Command::Diff(c) => handle_command_diff(c),
		Command::Man(c) => handle_command_man(c),
		Command::Completions(c) => handle_command_completions(c),
	}
}

/// Reads and creates a profile from a path.
fn setup_profile(
	profile_name: &str,
	source: &PunktfSource,
	target: Option<PathBuf>,
) -> Result<LayeredProfile> {
	let mut builder = LayeredProfile::build();

	// Add target cli argument to top
	let target_cli_profile = Profile {
		target,
		..Default::default()
	};
	builder.add(String::from("target_cli_argument"), target_cli_profile);

	resolve_profile(&mut builder, source, profile_name)?;

	// Add target environment variable to bottom
	let target_env_profile = Profile {
		target: util::get_target_path(),
		..Default::default()
	};
	builder.add(
		String::from("target_environment_variable"),
		target_env_profile,
	);

	Ok(builder.finish())
}

/// Sets up the environment with PUNKTF specific variables.
fn setup_env(source: &PunktfSource, profile: &LayeredProfile, profile_name: &str) {
	// Setup environment
	std::env::set_var("PUNKTF_CURRENT_SOURCE", source.root());
	if let Some(target) = profile.target_path() {
		std::env::set_var("PUNKTF_CURRENT_TARGET", target);
	}
	std::env::set_var("PUNKTF_CURRENT_PROFILE", profile_name);
}

/// Handles the writing of the deployment status to output files/formats.
fn handle_output(
	opt::OutputShared {
		json_output,
		yaml_output,
	}: opt::OutputShared,
	deployment: &Deployment,
) {
	/// Creates a new file. Fails if the file exists.
	///
	/// TODO: replace with `std/fs/struct.File.html#method.create_new` once stable.
	fn create_file(path: &Path) -> std::io::Result<File> {
		OpenOptions::new().create_new(true).write(true).open(path)
	}

	'json: {
		if let Some(json_path) = json_output {
			let mut file = match create_file(&json_path) {
				Ok(file) => file,
				Err(err) => {
					log::error!("Failed to create json output file: {err}");
					break 'json;
				}
			};

			if let Err(err) = serde_json::to_writer_pretty(&mut file, deployment) {
				log::error!("Failed to write deployment status to json output file: {err}");
				break 'json;
			}
		}
	}

	'yaml: {
		if let Some(yaml_path) = yaml_output {
			let mut file = match create_file(&yaml_path) {
				Ok(file) => file,
				Err(err) => {
					log::error!("Failed to create yaml output file: {err}");
					break 'yaml;
				}
			};

			if let Err(err) = serde_yaml::to_writer(&mut file, deployment) {
				log::error!("Failed to write deployment status to yaml output file: {err}");
				break 'yaml;
			}
		}
	}
}

/// Handles the `deploy` command processing.
fn handle_command_deploy(
	opt::Deploy {
		shared: opt::RepoShared {
			source,
			profile: profile_name,
			..
		},
		target,
		dry_run,
		output,
	}: opt::Deploy,
) -> Result<()> {
	let ptf_src = PunktfSource::from_root(source)?;
	let mut profile = setup_profile(&profile_name, &ptf_src, target)?;

	// Ensure target is set
	if profile.target_path().is_none() {
		panic!(
			"No target path for the deployment set. Either use the command line argument \
					 `-t/--target`, the profile attribute `target` or the environment variable \
					 `{PUNKTF_TARGET_ENVVAR}`"
		)
	}

	log::debug!("Profile:\n{:#?}", profile);
	log::debug!("Source: {}", ptf_src.root().display());
	log::debug!("Target: {:?}", profile.target_path());

	setup_env(&ptf_src, &profile, &profile_name);

	let options = DeployOptions { dry_run };
	let deployment = Deployer::new(options, util::ask_user_merge).deploy(&ptf_src, &mut profile);

	log::debug!("Deployment:\n{:#?}", deployment);
	util::log_deployment(&deployment, true);

	handle_output(output, &deployment);

	if options.dry_run {
		log::info!("Note: No files were actually deployed, since dry run mode was enabled");
	}

	if deployment.status().is_failed() {
		Err(eyre!("Some dotfiles failed to deploy"))
	} else {
		Ok(())
	}
}

/// Handles the `render` command processing.
fn handle_command_render(
	opt::Render {
		shared: opt::RepoShared {
			source,
			profile: profile_name,
			..
		},
		dotfile,
	}: opt::Render,
) -> Result<()> {
	/// Finds the relevant dotfile from which includes the file a the given
	/// `relative_source_path`.
	/// This is needed as some files might stem from a directory dotfile.
	fn find_dotfile<'a, 'b>(
		dotfiles: impl Iterator<Item = &'a Dotfile>,
		relative_source_path: &'b Path,
	) -> Option<&'a Dotfile> {
		dotfiles
			.filter_map(|d| {
				relative_source_path
					.strip_prefix(&d.path)
					.map(|p| (d, p))
					.ok()
			})
			.reduce(|a, i| {
				// First sort by tiniest difference to dotfile path
				// then by highest priority.
				match (i.1.as_os_str().len(), a.1.as_os_str().len()) {
					(i_len, a_len) if i_len == a_len => match (i.0.priority, a.0.priority) {
						(Some(ip), Some(ap)) if ip >= ap => i,
						(Some(_), None) | (None, None) => i,
						_ => a,
					},
					(i_len, a_len) if i_len < a_len => i,
					_ => a,
				}
			})
			.map(|(d, _)| d)
	}

	let ptf_src = PunktfSource::from_root(source)?;
	let profile = setup_profile(&profile_name, &ptf_src, None)?;

	log::debug!("Profile:\n{:#?}", profile);
	log::debug!("Source: {}", ptf_src.root().display());
	log::debug!("Target: {:?}", profile.target_path());

	setup_env(&ptf_src, &profile, &profile_name);

	let dotfile_vars = if let Some(dotfile) = find_dotfile(profile.dotfiles(), &dotfile) {
		log::debug!("Dotfile found in profile");
		dotfile.variables.as_ref()
	} else {
		log::warn!("Dotfile not found in profile");
		None
	};

	let file = ptf_src.dotfiles().join(dotfile);
	let content = std::fs::read_to_string(&file)?;
	let file_source = Source::file(&file, &content);
	let template = Template::parse(file_source)?;
	let resolved = template.resolve(Some(profile.variables()), dotfile_vars)?;

	print!("{resolved}");

	Ok(())
}

/// Handles the `verify` command processing.
///
/// This is basically a alias for `deploy --dry-run`.
fn handle_command_verify(
	opt::Verify {
		shared: opt::RepoShared {
			source,
			profile: profile_name,
			..
		},
		output,
	}: opt::Verify,
) -> Result<()> {
	let ptf_src = PunktfSource::from_root(source)?;
	let mut profile = setup_profile(&profile_name, &ptf_src, None)?;

	log::debug!("Profile:\n{:#?}", profile);
	log::debug!("Source: {}", ptf_src.root().display());
	log::debug!("Target: {:?}", profile.target_path());

	setup_env(&ptf_src, &profile, &profile_name);

	let options = DeployOptions { dry_run: true };
	let deployment = Deployer::new(options, util::ask_user_merge).deploy(&ptf_src, &mut profile);

	log::debug!("Deployment:\n{:#?}", deployment);
	util::log_deployment(&deployment, true);

	handle_output(output, &deployment);

	Ok(())
}

/// Handles the `diff` command processing.
fn handle_command_diff(
	opt::Diff {
		shared: opt::RepoShared {
			source,
			profile: profile_name,
			..
		},
		format,
	}: opt::Diff,
) -> Result<()> {
	let ptf_src = PunktfSource::from_root(source)?;
	let mut profile = setup_profile(&profile_name, &ptf_src, None)?;

	log::debug!("Profile:\n{:#?}", profile);
	log::debug!("Source: {}", ptf_src.root().display());
	log::debug!("Target: {:?}", profile.target_path());

	setup_env(&ptf_src, &profile, &profile_name);

	Diff::new(|event| diff::diff(format, event)).diff(&ptf_src, &mut profile);

	Ok(())
}

/// Handles the `man` command processing.
fn handle_command_man(opt::Man { output }: opt::Man) -> Result<()> {
	let output = output.join(format!("{BINARY_NAME}.1"));

	let man = clap_mangen::Man::new(opt::Opts::command());
	let mut buffer: Vec<u8> = Default::default();
	man.render(&mut buffer)?;

	std::fs::write(output, buffer)?;

	Ok(())
}

/// Handles the `completions` command processing.
fn handle_command_completions(opt::Completions { shell, output }: opt::Completions) -> Result<()> {
	clap_complete::generate_to(shell, &mut opt::Opts::command(), BINARY_NAME, output)?;

	Ok(())
}
