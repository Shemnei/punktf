use core::fmt;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::{crate_authors, crate_description, crate_version, Clap};
use color_eyre::eyre::Context;
use color_eyre::owo_colors::OwoColorize;
use color_eyre::Result;
use log::debug;
use punktf::deploy::deployment::{Deployment, DeploymentStatus};
use punktf::deploy::executor::{Executor, ExecutorOptions};
use punktf::deploy::item::ItemStatus;
use punktf::{resolve_profile, Profile};

// Used so that it defaults to current_dir if no value is given.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct SourcePath(PathBuf);

impl Default for SourcePath {
	fn default() -> Self {
		Self(std::env::current_dir().expect(
			"Failed to get `current_dir`. Please either use the `-s/--source` argument or the \
			 environment variable `PUNKTF_SOURCE` to set the source directory.",
		))
	}
}

impl fmt::Display for SourcePath {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Display::fmt(&self.0.display(), f)
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
	#[clap(short, long, env = "PUNKTF_SOURCE", default_value)]
	source: SourcePath,

	#[clap(short, long, parse(from_occurrences))]
	verbose: u8,
}

#[derive(Debug, Clap)]
enum Command {
	Deploy(Deploy),
}

#[derive(Debug, Clap)]
struct Deploy {
	profile: String,

	#[clap(short, long)]
	dry_run: bool,
}

// TODO: target path as cli arg
// TODO: cleanup/improve stdout output
// TODO: option to output deployment struct to file

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

	debug!("Parsed Opts: {:#?}", opts);

	handle_commands(opts)
}

fn handle_commands(opts: Opts) -> Result<()> {
	match opts.command {
		Command::Deploy(cmd) => {
			let profile_path = opts.shared.source.join("profiles");

			let profile: Profile = resolve_profile(&profile_path, &cmd.profile)?;

			debug!("Profile: {:#?}", profile);
			debug!("{}", serde_json::to_string_pretty(&profile).unwrap());

			let options = ExecutorOptions {
				dry_run: cmd.dry_run,
			};

			let deployer = Executor::new(options, ask_user_merge);

			let deployment = deployer
				.deploy(opts.shared.source.0, profile)
				.wrap_err("Failed to deploy");

			match deployment {
				Ok(deployment) => {
					log::debug!("{:#?}", deployment);
					log_deployment(deployment);
				}
				Err(err) => {
					log::error!("Failed to deploy: {}", err);
				}
			};
		}
	}

	Ok(())
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

fn log_deployment(deployment: Deployment) {
	let mut out = String::new();

	let mut files_success = 0;
	for (idx, (path, _)) in deployment
		.items()
		.iter()
		.filter(|(_, v)| v.status().is_success())
		.enumerate()
	{
		if idx == 0 {
			out.push_str(&format!("ITEMS ({})", "SUCCESS".green()));
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
		.items()
		.iter()
		.filter_map(|(k, v)| {
			if let ItemStatus::Skipped(reason) = v.status() {
				Some((k, reason))
			} else {
				None
			}
		})
		.enumerate()
	{
		if idx == 0 {
			out.push_str(&format!("\nITEMS ({})", "SKIPPED".yellow()));
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
		.items()
		.iter()
		.filter_map(|(k, v)| {
			if let ItemStatus::Failed(reason) = v.status() {
				Some((k, reason))
			} else {
				None
			}
		})
		.enumerate()
	{
		if idx == 0 {
			out.push_str(&format!("ITEMS ({})", "FAILED".red()));
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
