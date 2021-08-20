//! All code related to command line argument parsing.

// We allow missing documentation for this module, as any documentation put on
// the cli struct will appear in the help message which, in most cases, is not
// what we want.
#![allow(missing_docs, clippy::missing_docs_in_private_items)]

use std::fmt;
use std::ops::Deref;
use std::path::PathBuf;
use std::str::FromStr;

use clap::{crate_authors, crate_description, crate_version, Clap};
use color_eyre::Result;

/// The path to `punktfs` source directory.
///
/// Used so that it defaults to [`std::env::current_dir`] if no value is given.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourcePath(PathBuf);

impl Default for SourcePath {
	fn default() -> Self {
		Self(std::env::current_dir().unwrap_or_else(|_| {
			panic!(
				"Failed to get `current_dir`. Please either use the `-s/--source` argument or the \
				 environment variable `{}` to set the source directory.",
				super::PUNKTF_SOURCE_ENVVAR
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
pub struct Opts {
	#[clap(flatten)]
	pub shared: Shared,

	#[clap(subcommand)]
	pub command: Command,
}

#[derive(Debug, Clap)]
pub struct Shared {
	/// The source directory where the profiles and dotfiles are located.
	#[clap(short, long, env = super::PUNKTF_SOURCE_ENVVAR, default_value_t)]
	pub source: SourcePath,

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
