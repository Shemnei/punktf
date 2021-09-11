//! All code related to command line argument parsing.

// We allow missing documentation for this module, as any documentation put on
// the cli struct will appear in the help message which, in most cases, is not
// what we want.
#![allow(missing_docs, clippy::missing_docs_in_private_items)]

use std::path::PathBuf;

use clap::{crate_authors, crate_description, crate_version, Clap};

#[derive(Debug, Clap)]
#[clap(version = crate_version!(), author = crate_authors!(), about = crate_description!())]
pub struct Opts {
	#[clap(flatten)]
	pub shared: Shared,

	#[clap(subcommand)]
	pub command: Command,
}

#[derive(Debug, Clap)]
pub struct Shared {
	/// The source directory where the profiles and dotfiles are located.
	#[clap(short, long, env = super::PUNKTF_SOURCE_ENVVAR)]
	pub source: PathBuf,

	/// Runs with specified level of verbosity which affects the log level.
	///
	/// The level can be set by repeating the flag `n` times (e.g. `-vv` for 2).
	/// Levels:
	///     0 - `Info`;
	///     1 - `Debug`;
	///     2 - `Trace`.
	#[clap(short, long, parse(from_occurrences))]
	pub verbose: u8,
}

#[derive(Debug, Clap)]
pub enum Command {
	Deploy(Deploy),
}

/// Deploys a profile.
#[derive(Debug, Clap)]
pub struct Deploy {
	/// Name of the profile to deploy.
	///
	/// The name should be the file name of the profile without an extension (e.g.
	/// `profiles/arch.json` should be given as `arch`).
	#[clap(env = super::PUNKTF_PROFILE_ENVVAR)]
	pub profile: String,

	/// Alternative deployment target path.
	///
	/// This path will take precedence over all other ways to define a deployment
	/// path.
	#[clap(short, long)]
	pub target: Option<PathBuf>,

	/// Deploys the profile but without actually coping/creating the files.
	///
	/// This can be used to test and get an overview over the changes which would
	/// be applied when run without this flag.
	#[clap(short, long)]
	pub dry_run: bool,
}
