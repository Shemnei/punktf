use core::fmt;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::{crate_authors, crate_description, crate_version, Clap};
use color_eyre::eyre::eyre;
use color_eyre::owo_colors::OwoColorize;
use color_eyre::Result;
use punktf::deploy::deployment::{Deployment, DeploymentStatus};
use punktf::deploy::dotfile::DotfileStatus;
use punktf::deploy::executor::{Executor, ExecutorOptions};
use punktf::profile::{resolve_profile, LayeredProfile, SimpleProfile};
use punktf::PunktfSource;

const PUNKTF_SOURCE_ENVVAR: &str = "PUNKTF_SOURCE";
const PUNKTF_TARGET_ENVVAR: &str = "PUNKTF_TARGET";
const PUNKTF_DEFAULT_PROFILE_ENVVAR: &str = "PUNKTF_PROFILE";

fn get_target_path() -> PathBuf {
	std::env::var_os(PUNKTF_TARGET_ENVVAR)
		.unwrap_or_else(|| {
			panic!(
				"No environment variable `{}` set. Either set this variable, use the `-t/--target`
				argument or use the profile attribute `target`.",
				PUNKTF_TARGET_ENVVAR
			)
		})
		.into()
}

// Used so that it defaults to current_dir if no value is given.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct SourcePath(PathBuf);

impl Default for SourcePath {
	fn default() -> Self {
		Self(std::env::current_dir().unwrap_or_else(|_| {
			panic!(
				"Failed to get `current_dir`. Please either use the `-s/--source` argument or the \
				 environment variable `{}` to set the source directory.",
				PUNKTF_SOURCE_ENVVAR
			)
		}))
	}
}

impl fmt::Display for SourcePath {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Display::fmt(&self.0.display(), f)
	}
}

impl From<SourcePath> for PathBuf {
	fn from(value: SourcePath) -> Self {
		value.0
	}
}

impl Deref for SourcePath {
	type Target = PathBuf;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl FromStr for SourcePath {
	type Err = std::convert::Infallible;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(Self(PathBuf::from(s)))
	}
}

#[derive(Debug, Clap)]
#[clap(version = crate_version!(), author = crate_authors!(), about = crate_description!())]
struct Opts {
	#[clap(flatten)]
	shared: Shared,

	#[clap(subcommand)]
	command: Command,
}

#[derive(Debug, Clap)]
struct Shared {
	/// The source directory where the profiles and dotfiles are located.
	#[clap(short, long, env = PUNKTF_SOURCE_ENVVAR)]
	// The below is necessary for as `clap` will act different when debugging vs
	// releasing. This will either cause `cargo test` or `cargo install` to fail.
	// clap = "3.0.0-beta.2"
	#[cfg_attr(debug_assertions, clap(default_value))]
	#[cfg_attr(not(debug_assertions), clap(default_value_t))]
	source: SourcePath,

	/// Runs with specified level of verbosity which affects the log level.
	///
	/// The level can be set by repeating the flag `n` times (e.g. `-vv` for 2).
	/// Levels:
	///     0 - `Info`;
	///     1 - `Debug`;
	///     2 - `Trace`.
	#[clap(short, long, parse(from_occurrences))]
	verbose: u8,
}

#[derive(Debug, Clap)]
enum Command {
	Deploy(Deploy),
}

/// Deploys a profile.
#[derive(Debug, Clap)]
struct Deploy {
	/// Name of the profile to deploy.
	///
	/// The name should be the file name of the profile without an extension (e.g.
	/// `profiles/arch.json` should be given as `arch`).
	#[clap(env = PUNKTF_DEFAULT_PROFILE_ENVVAR)]
	profile: String,

	/// Alternative deployment target path.
	///
	/// This path will take precedence over all other ways to define a deployment
	/// path.
	#[clap(short, long)]
	target: Option<PathBuf>,

	/// Deploys the profile but without actually coping/creating the files.
	///
	/// This can be used to test and get an overview over the changes which would
	/// be applied when run without this flag.
	#[clap(short, long)]
	dry_run: bool,
}

fn main() -> Result<()> {
	let _ = color_eyre::install()?;

	let opts: Opts = Opts::parse();

	let log_level = match opts.shared.verbose {
		0 => log::Level::Info,
		1 => log::Level::Debug,
		_ => log::Level::Trace,
	};

	let _ = env_logger::Builder::from_env(
		env_logger::Env::default().default_filter_or(log_level.as_str()),
	)
	.init();

	log::debug!("Parsed Opts:\n{:#?}", opts);

	handle_commands(opts)
}

fn handle_commands(opts: Opts) -> Result<()> {
	match opts.command {
		Command::Deploy(cmd) => {
			let ptf_src = PunktfSource::from_root(opts.shared.source.into())?;

			let mut builder = LayeredProfile::build();

			// Add target cli argument to top
			let target_cli_profile = SimpleProfile {
				target: cmd.target,
				..Default::default()
			};
			builder.add(String::from("target_cli_argument"), target_cli_profile);

			resolve_profile(
				&mut builder,
				&ptf_src,
				&cmd.profile,
				&mut Default::default(),
			)?;

			// Add target environment variable to bottom
			let target_env_profile = SimpleProfile {
				target: Some(get_target_path()),
				..Default::default()
			};
			builder.add(
				String::from("target_environment_variable"),
				target_env_profile,
			);

			let profile = builder.finish();

			log::debug!("Profile:\n{:#?}", profile);
			log::debug!("Source: {}", ptf_src.root().display());
			log::debug!("Target: {:?}", profile.target_path());

			// Setup environment
			std::env::set_var("PUNKTF_CURRENT_SOURCE", ptf_src.root());
			if let Some(target) = profile.target_path() {
				std::env::set_var("PUNKTF_CURRENT_TARGET", target);
			}
			std::env::set_var("PUNKTF_CURRENT_PROFILE", cmd.profile);

			let options = ExecutorOptions {
				dry_run: cmd.dry_run,
			};

			let deployer = Executor::new(options, ask_user_merge);

			let deployment = deployer.deploy(ptf_src, &profile);

			match deployment {
				Ok(deployment) => {
					log::debug!("Deployment:\n{:#?}", deployment);
					log_deployment(&deployment);

					if deployment.status().is_failed() {
						Err(eyre!("Some dotfiles failed to deploy"))
					} else {
						Ok(())
					}
				}
				Err(err) => {
					log::error!("Deployment aborted: {}", err);
					Err(err)
				}
			}
		}
	}
}

fn ask_user_merge(source_path: &Path, deploy_path: &Path) -> Result<bool> {
	use std::io::Write;

	let stdin = std::io::stdin();
	let mut stdout = std::io::stdout();
	let mut line = String::new();

	loop {
		stdout.write_all(
			format!(
				"Overwrite `{}` with `{}` [y/N]: ",
				deploy_path.display(),
				source_path.display()
			)
			.as_bytes(),
		)?;

		stdout.flush()?;

		stdin.read_line(&mut line)?;

		line.make_ascii_lowercase();

		return match line.trim() {
			"y" => Ok(true),
			"n" => Ok(false),
			_ => {
				line.clear();
				continue;
			}
		};
	}
}

fn log_deployment(deployment: &Deployment) {
	let mut out = String::new();

	let mut files_success = 0;
	for (idx, (path, _)) in deployment
		.dotfiles()
		.iter()
		.filter(|(_, v)| v.status().is_success())
		.enumerate()
	{
		if idx == 0 {
			out.push_str(&format!("Dotfiles ({})", "SUCCESS".green()));
		}

		out.push_str(&format!("\n\t{}", path.display().bright_black()));
		files_success += 1;
	}

	if !out.is_empty() {
		log::info!("{}", out);
		out.clear();
	}

	let mut files_skipped = 0;
	for (idx, (path, reason)) in deployment
		.dotfiles()
		.iter()
		.filter_map(|(k, v)| {
			if let DotfileStatus::Skipped(reason) = v.status() {
				Some((k, reason))
			} else {
				None
			}
		})
		.enumerate()
	{
		if idx == 0 {
			out.push_str(&format!("Dotfiles ({})", "SKIPPED".yellow()));
		}

		out.push_str(&format!(
			"\n\t{}: {}",
			path.display(),
			reason.bright_black()
		));
		files_skipped += 1;
	}

	if !out.is_empty() {
		log::warn!("{}", out);
		out.clear();
	}

	let mut files_failed = 0;
	for (idx, (path, reason)) in deployment
		.dotfiles()
		.iter()
		.filter_map(|(k, v)| {
			if let DotfileStatus::Failed(reason) = v.status() {
				Some((k, reason))
			} else {
				None
			}
		})
		.enumerate()
	{
		if idx == 0 {
			out.push_str(&format!("Dotfiles ({})", "FAILED".red()));
		}

		out.push_str(&format!(
			"\n\t{}: {}",
			path.display(),
			reason.bright_black()
		));
		files_failed += 1;
	}

	if !out.is_empty() {
		log::error!("{}", out);
		out.clear();
	}

	match deployment.status() {
		DeploymentStatus::Success => {
			out.push_str(&format!("Status: {}", "SUCCESS".green()));
		}
		DeploymentStatus::Failed(reason) => {
			out.push_str(&format!("Status: {}\n\t{}", "FAILED".red(), reason));
		}
	};

	let files_total = files_success + files_skipped + files_failed;
	let elapsed = deployment
		.duration()
		.expect("Failed to get duration from deployment");

	out.push_str(&format!("\nTime            : {:?}", elapsed));
	out.push_str(&format!("\nFiles (deployed): {}", files_success));
	out.push_str(&format!("\nFiles (skipped) : {}", files_skipped));
	out.push_str(&format!("\nFiles (failed)  : {}", files_failed));
	out.push_str(&format!("\nFiles (total)   : {}", files_total));

	log::info!("{}", out);
}
