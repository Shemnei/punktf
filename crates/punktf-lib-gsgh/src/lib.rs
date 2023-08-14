pub mod version {
	use serde::{de::Visitor, Deserialize, Deserializer, Serialize};
	use std::{fmt, num::ParseIntError, str::FromStr};
	use thiserror::Error;

	#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
	pub enum ParseVersionError {
		#[error("trailing characters")]
		TrailingCharacters,
		#[error("invalid number")]
		InvalidNumber,
		#[error("empty")]
		Empty,
		#[error("invalid separator")]
		InvalidSeparator,
	}

	impl From<ParseIntError> for ParseVersionError {
		fn from(_: ParseIntError) -> Self {
			Self::InvalidNumber
		}
	}

	pub type Result<T, E = ParseVersionError> = std::result::Result<T, E>;

	#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
	pub struct Version {
		pub major: u8,
		pub minor: u8,
		pub patch: u8,
	}

	impl fmt::Display for Version {
		fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
			let Self {
				major,
				minor,
				patch,
			} = self;

			write!(f, "{major}.{minor}.{patch}")
		}
	}

	impl Version {
		pub const ZERO: Self = Version {
			major: 0,
			minor: 0,
			patch: 0,
		};

		pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
			Self {
				major,
				minor,
				patch,
			}
		}

		pub const fn with_major(mut self, major: u8) -> Self {
			self.major = major;
			self
		}

		pub const fn with_minor(mut self, minor: u8) -> Self {
			self.minor = minor;
			self
		}

		pub const fn with_patch(mut self, patch: u8) -> Self {
			self.patch = patch;
			self
		}

		pub const fn compatible(self, other: Self) -> bool {
			self.major == other.major
		}
	}

	fn parse_u8(s: &str) -> Result<Option<(&str, u8)>> {
		fn check_digit(bytes: &[u8], idx: usize) -> bool {
			bytes.get(idx).map(u8::is_ascii_digit).unwrap_or(false)
		}

		if s.is_empty() {
			return Ok(None);
		}

		let bytes = s.as_bytes();

		// u8 can be max 3 digits (255)
		let eat = match (
			check_digit(bytes, 0),
			check_digit(bytes, 1),
			check_digit(bytes, 2),
		) {
			(true, true, true) => 3,
			(true, true, _) => 2,
			(true, _, _) => 1,
			_ => return Err(ParseVersionError::InvalidNumber),
		};

		Ok(Some((&s[eat..], s[..eat].parse::<u8>()?)))
	}

	fn parse_dot(s: &str) -> Result<Option<&str>> {
		if s.is_empty() {
			return Ok(None);
		}

		if s.as_bytes()[0] == b'.' {
			Ok(Some(&s[1..]))
		} else {
			Err(ParseVersionError::InvalidSeparator)
		}
	}

	impl FromStr for Version {
		type Err = ParseVersionError;

		fn from_str(s: &str) -> Result<Self, Self::Err> {
			let Some((s, major)) = parse_u8(s)? else {
				return Err(ParseVersionError::Empty);
			};

			let Some(s) = parse_dot(s)? else {
				return Ok(Version { major, ..Default::default() });
			};

			let Some((s, minor)) = parse_u8(s)? else {
				// The parse `.` is trailing
				return Err(ParseVersionError::TrailingCharacters);
			};

			let Some(s) = parse_dot(s)? else {
				return Ok(Version { major, minor, ..Default::default() });
			};

			let Some((s, patch)) = parse_u8(s)? else {
				// The parse `.` is trailing
				return Err(ParseVersionError::TrailingCharacters);
			};

			if s.is_empty() {
				Ok(Version {
					major,
					minor,
					patch,
				})
			} else {
				Err(ParseVersionError::TrailingCharacters)
			}
		}
	}

	impl Serialize for Version {
		fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
		where
			S: serde::Serializer,
		{
			serializer.serialize_str(&self.to_string())
		}
	}

	impl<'de> Deserialize<'de> for Version {
		fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
		where
			D: Deserializer<'de>,
		{
			struct VersionVisitor;

			impl<'de> Visitor<'de> for VersionVisitor {
				type Value = Version;

				fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
					formatter.write_str("semver version")
				}

				fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
				where
					E: serde::de::Error,
				{
					string.parse().map_err(serde::de::Error::custom)
				}
			}

			deserializer.deserialize_str(VersionVisitor)
		}
	}

	#[cfg(test)]
	mod tests {
		use super::*;

		#[test]
		fn version_parse_ok() -> Result<(), Box<dyn std::error::Error>> {
			assert_eq!("1".parse::<Version>()?, Version::ZERO.with_major(1));
			assert_eq!(
				"22.12".parse::<Version>()?,
				Version::ZERO.with_major(22).with_minor(12)
			);
			assert_eq!(
				"0.12.55".parse::<Version>()?,
				Version::ZERO.with_minor(12).with_patch(55)
			);
			Ok(())
		}

		#[test]
		fn version_parse_err() -> Result<(), Box<dyn std::error::Error>> {
			assert_eq!(
				"1.".parse::<Version>(),
				Err(ParseVersionError::TrailingCharacters)
			);
			assert_eq!("".parse::<Version>(), Err(ParseVersionError::Empty));
			assert_eq!(
				"1.1.1 ".parse::<Version>(),
				Err(ParseVersionError::TrailingCharacters)
			);
			assert_eq!(
				"1.1.1.".parse::<Version>(),
				Err(ParseVersionError::TrailingCharacters)
			);
			assert_eq!(
				"1.1.1.1".parse::<Version>(),
				Err(ParseVersionError::TrailingCharacters)
			);
			assert_eq!(
				"256".parse::<Version>(),
				Err(ParseVersionError::InvalidNumber)
			);

			Ok(())
		}

		#[test]
		fn version_cmp() {
			assert!(Version::ZERO.with_major(1) == Version::ZERO.with_major(1));
			assert!(Version::ZERO.with_minor(2) == Version::ZERO.with_minor(2));
			assert!(Version::ZERO.with_patch(3) == Version::ZERO.with_patch(3));

			assert!(Version::ZERO.with_major(1) < Version::ZERO.with_major(2));
			assert!(Version::ZERO.with_minor(2) < Version::ZERO.with_major(2));
			assert!(Version::ZERO.with_patch(3) < Version::ZERO.with_major(2));

			assert!(
				Version {
					major: 2,
					minor: 3,
					patch: 1
				} > Version {
					major: 2,
					minor: 1,
					patch: 10
				}
			);
		}
	}
}

pub mod env {
	use std::{
		collections::{btree_set, BTreeMap, BTreeSet, HashSet},
		ops::Deref,
	};

	use serde::{Deserialize, Serialize};

	#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
	pub struct Environment(pub BTreeMap<String, serde_yaml::Value>);

	impl Environment {
		pub fn is_empty(&self) -> bool {
			self.0.is_empty()
		}
	}

	#[derive(Default, Debug, Clone, PartialEq, Eq)]
	pub struct LayeredEnvironment(Vec<(&'static str, Environment)>);

	impl LayeredEnvironment {
		pub fn push(&mut self, name: &'static str, env: Environment) {
			self.0.push((name, env));
		}

		pub fn pop(&mut self) -> Option<(&'static str, Environment)> {
			self.0.pop()
		}

		pub fn keys(&self) -> BTreeSet<&str> {
			self.0
				.iter()
				.flat_map(|(_, layer)| layer.0.keys())
				.map(|key| key.as_str())
				.collect()
		}

		pub fn get(&self, key: &str) -> Option<&serde_yaml::Value> {
			for (_, layer) in self.0.iter() {
				if let Some(value) = layer.0.get(key) {
					return Some(value);
				}
			}

			return None;
		}

		pub fn iter(&self) -> LayeredIter<'_> {
			LayeredIter::new(self)
		}

		pub fn as_str_map(&self) -> BTreeMap<&str, String> {
			self.iter()
				// TODO: Optimize
				// `trim` to remove trailing `\n`
				.map(|(k, v)| (k, serde_yaml::to_string(v).unwrap().trim().into()))
				.collect()
		}
	}

	pub struct LayeredIter<'a> {
		env: &'a LayeredEnvironment,
		keys: btree_set::IntoIter<&'a str>,
	}

	impl<'a> LayeredIter<'a> {
		pub fn new(env: &'a LayeredEnvironment) -> Self {
			let keys = env.keys().into_iter();
			Self { env, keys }
		}
	}

	impl<'a> Iterator for LayeredIter<'a> {
		type Item = (&'a str, &'a serde_yaml::Value);

		fn next(&mut self) -> Option<Self::Item> {
			let key = self.keys.next()?;
			Some((key, self.env.get(key)?))
		}
	}
}

pub mod transform {
	use serde::{Deserialize, Serialize};

	pub trait Transform {
		fn apply(&self, content: String) -> Result<String, Box<dyn std::error::Error>>;
	}

	#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
	#[serde(tag = "type", content = "with", rename_all = "snake_case")]
	pub enum Transformer {
		/// Transformer which replaces line termination characters with either unix
		/// style (`\n`) or windows style (`\r\b`).
		LineTerminator(LineTerminator),
	}

	/// Transformer which replaces line termination characters with either unix
	/// style (`\n`) or windows style (`\r\b`).
	#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
	pub enum LineTerminator {
		/// Replaces all occurrences of `\r\n` with `\n` (unix style).
		LF,

		/// Replaces all occurrences of `\n` with `\r\n` (windows style).
		CRLF,
	}
}

pub mod hook {
	use std::{
		io::{BufRead, BufReader},
		path::{Path, PathBuf},
		process::{Command, Stdio},
	};

	use serde::{Deserialize, Serialize};
	use thiserror::Error;

	use crate::env::LayeredEnvironment;

	// Have special syntax for skipping deployment on pre_hook
	// Analog: <https://learn.microsoft.com/en-us/azure/devops/pipelines/scripts/logging-commands?view=azure-devops>
	// e.g. punktf:skip_deployment

	#[derive(Error, Debug)]
	pub enum HookError {
		#[error("IO Error")]
		IoError(#[from] std::io::Error),

		#[error("Process failed with status `{0}`")]
		ExitStatusError(std::process::ExitStatus),
	}

	impl From<std::process::ExitStatus> for HookError {
		fn from(value: std::process::ExitStatus) -> Self {
			Self::ExitStatusError(value)
		}
	}

	pub type Result<T, E = HookError> = std::result::Result<T, E>;

	// TODO: Replace once `exit_ok` becomes stable
	trait ExitOk {
		type Error;

		fn exit_ok(self) -> Result<(), Self::Error>;
	}

	impl ExitOk for std::process::ExitStatus {
		type Error = HookError;

		fn exit_ok(self) -> Result<(), <Self as ExitOk>::Error> {
			if self.success() {
				Ok(())
			} else {
				Err(self.into())
			}
		}
	}

	#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
	#[serde(tag = "type", content = "with", rename_all = "snake_case")]
	pub enum Hook {
		Inline(String),
		File(PathBuf),
	}

	impl Hook {
		pub fn run(self, cwd: &Path, env: LayeredEnvironment) -> Result<()> {
			let mut child = self
				.prepare_command()?
				.current_dir(cwd)
				.stdout(Stdio::piped())
				.stderr(Stdio::piped())
				.envs(env.as_str_map())
				.spawn()?;

			// No need to call kill here as the program will immediately exit
			// and thereby kill all spawned children
			let stdout = child.stdout.take().expect("Failed to get stdout from hook");

			for line in BufReader::new(stdout).lines() {
				match line {
					Ok(line) => println!("hook::stdout > {}", line),
					Err(err) => {
						// Result is explicitly ignored as an error was already
						// encountered
						let _ = child.kill();
						return Err(err.into());
					}
				}
			}

			// No need to call kill here as the program will immediately exit
			// and thereby kill all spawned children
			let stderr = child.stderr.take().expect("Failed to get stderr from hook");

			for line in BufReader::new(stderr).lines() {
				match line {
					Ok(line) => println!("hook::stderr > {}", line),
					Err(err) => {
						// Result is explicitly ignored as an error was already
						// encountered
						let _ = child.kill();
						return Err(err.into());
					}
				}
			}

			child
				.wait_with_output()?
				.status
				.exit_ok()
				.map_err(Into::into)
		}

		fn prepare_command(&self) -> Result<Command> {
			let mut cmd = None;

			#[cfg(target_family = "windows")]
			{
				let mut c = Command::new("cmd");
				c.arg("/C");
				cmd = Some(c);
			}

			#[cfg(target_family = "unix")]
			{
				let mut c = Command::new("sh");
				c.arg("-c");
				cmd = Some(c)
			}

			let Some(mut cmd) = cmd else {
				return Err(HookError::IoError(std::io::Error::new(std::io::ErrorKind::Other, "Hooks are only supported on Windows and Unix-based systems")));
			};

			match self {
				Self::Inline(s) => {
					cmd.arg(s);
				}
				Self::File(path) => {
					let s = std::fs::read_to_string(path)?;
					cmd.arg(s);
				}
			}

			Ok(cmd)
		}
	}

	#[cfg(test)]
	mod tests {
		use crate::env::Environment;

		use super::*;

		#[test]
		fn echo_hello_world() {
			let env = Environment(
				[
					("TEST", serde_yaml::Value::Bool(true)),
					("FOO", serde_yaml::Value::String(" BAR Test".into())),
				]
				.into_iter()
				.map(|(k, v)| (k.to_string(), v))
				.collect(),
			);

			let mut lenv = LayeredEnvironment::default();
			lenv.push("test", env);

			println!("{:#?}", lenv.as_str_map());

			let hook = Hook::Inline(r#"echo "Hello World""#.to_string());
			hook.run(Path::new("/tmp"), lenv).unwrap();
		}
	}
}

pub mod merge {
	use serde::{Deserialize, Serialize};

	use crate::hook::Hook;

	#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
	#[serde(tag = "type", content = "with", rename_all = "snake_case")]
	pub enum MergeMode {
		Hook(Hook),
	}
}

pub mod item {
	use std::path::PathBuf;

	use serde::{Deserialize, Serialize};

	use crate::{merge::MergeMode, profile::Shared};

	#[derive(Debug, Serialize, Deserialize)]
	pub struct Item {
		#[serde(flatten)]
		pub shared: Shared,

		pub path: PathBuf,

		#[serde(skip_serializing_if = "Option::is_none", default)]
		pub rename: Option<PathBuf>,

		#[serde(skip_serializing_if = "Option::is_none", default)]
		pub overwrite_target: Option<PathBuf>,

		#[serde(skip_serializing_if = "Option::is_none", default)]
		pub merge: Option<MergeMode>,
	}
}

pub mod prio {
	use serde::{Deserialize, Serialize};

	#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
	pub struct Priority(pub u32);

	impl PartialOrd for Priority {
		fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
			// Reverse sort ordering (smaller = higher)
			other.0.partial_cmp(&self.0)
		}
	}

	impl Ord for Priority {
		fn cmp(&self, other: &Self) -> std::cmp::Ordering {
			// Reverse sort ordering (smaller = higher)
			other.0.cmp(&self.0)
		}
	}
}

pub mod profile {
	use crate::{
		env::Environment, hook::Hook, item::Item, prio::Priority, transform::Transformer,
		version::Version,
	};
	use std::{path::PathBuf, str::FromStr};

	use serde::{Deserialize, Serialize};
	use thiserror::Error;

	#[derive(Debug, Error)]
	pub enum Error {
		#[error("invalid profile: {0}")]
		InvalidProfile(#[from] serde_yaml::Error),
		#[error("unsupported version: {0}")]
		UnsupportedVersion(Version),
	}

	pub type Result<T, E = Error> = std::result::Result<T, E>;

	/// Wrapper struct to be able to first parse only the version and then choose
	/// the appropriate profile struct for it to do version compatible parsing.
	#[repr(transparent)]
	#[derive(Debug, Deserialize, Serialize)]
	#[serde(default)]
	pub struct ProfileVersion {
		pub version: Version,
	}

	impl Default for ProfileVersion {
		fn default() -> Self {
			Self {
				version: Version::ZERO,
			}
		}
	}

	impl From<ProfileVersion> for Version {
		fn from(value: ProfileVersion) -> Self {
			value.version
		}
	}

	impl AsRef<Version> for ProfileVersion {
		fn as_ref(&self) -> &Version {
			&self.version
		}
	}

	#[derive(Debug, Deserialize, Serialize)]
	pub struct Shared {
		#[serde(skip_serializing_if = "Option::is_none", default)]
		pub priority: Option<Priority>,

		#[serde(rename = "env", skip_serializing_if = "Environment::is_empty", default)]
		pub environment: Environment,

		#[serde(skip_serializing_if = "Vec::is_empty", default)]
		pub transformers: Vec<Transformer>,

		#[serde(skip_serializing_if = "Option::is_none", default)]
		pub pre_hook: Option<Hook>,

		#[serde(skip_serializing_if = "Option::is_none", default)]
		pub post_hook: Option<Hook>,
	}

	#[derive(Debug, Deserialize, Serialize)]
	pub struct Profile {
		#[serde(flatten)]
		pub version: ProfileVersion,

		#[serde(flatten)]
		pub shared: Shared,

		#[serde(skip_serializing_if = "Vec::is_empty", default)]
		pub aliases: Vec<String>,

		#[serde(skip_serializing_if = "Vec::is_empty", default)]
		pub extends: Vec<String>,

		#[serde(skip_serializing_if = "Option::is_none", default)]
		pub target: Option<PathBuf>,

		#[serde(skip_serializing_if = "Vec::is_empty", default)]
		pub items: Vec<Item>,
	}

	impl Profile {
		pub const VERSION: Version = Version::new(1, 0, 0);
	}

	impl FromStr for Profile {
		type Err = Error;

		fn from_str(s: &str) -> Result<Self> {
			let version: Version = serde_yaml::from_str::<ProfileVersion>(s)?.version;

			// No version or explicit zero version
			if version == Version::ZERO {
				return Err(Error::UnsupportedVersion(version));
			}

			// Version matching
			if Self::VERSION.compatible(version) {
				serde_yaml::from_str(s).map_err(Into::into)
			} else {
				Err(Error::UnsupportedVersion(version))
			}
		}
	}
}

#[test]
#[ignore = "debugging"]
fn main() -> Result<(), Box<dyn std::error::Error>> {
	use std::str::FromStr;

	let profile = std::fs::read_to_string("profile.yaml")?;
	let p = profile::Profile::from_str(&profile)?;

	println!("{p:#?}");

	Ok(())
}

#[test]
#[ignore = "debugging"]
fn prnp() {
	use crate::hook::Hook;
	use crate::{item::Item, prio::Priority};
	use env::Environment;
	use profile::{Profile, ProfileVersion};
	use serde_yaml::Value;
	use std::path::PathBuf;
	use transform::Transformer;

	use crate::profile::Shared;

	let p = Profile {
		version: ProfileVersion {
			version: Profile::VERSION,
		},
		aliases: vec!["Foo".into(), "Bar".into()],
		extends: vec!["Parent".into()],
		target: Some(PathBuf::from("Test")),
		shared: Shared {
			environment: Environment(
				[
					("Foo".into(), Value::String("Bar".into())),
					("Bool".into(), Value::Bool(true)),
				]
				.into_iter()
				.collect(),
			),
			transformers: vec![Transformer::LineTerminator(transform::LineTerminator::LF)],
			pre_hook: Some(Hook::Inline("set -eoux pipefail\necho 'Foo'".into())),
			post_hook: Some(Hook::File("Test".into())),
			priority: Some(Priority(5)),
		},

		items: vec![Item {
			shared: Shared {
				environment: Environment(
					[
						("Foo".into(), Value::String("Bar".into())),
						("Bool".into(), Value::Bool(true)),
					]
					.into_iter()
					.collect(),
				),
				transformers: vec![Transformer::LineTerminator(transform::LineTerminator::LF)],
				pre_hook: None,
				post_hook: None,
				priority: Some(Priority(5)),
			},
			path: PathBuf::from("/dev/null"),
			rename: None,
			overwrite_target: None,
			merge: None,
		}],
	};

	serde_yaml::to_writer(std::io::stdout(), &p).unwrap();
}

#[test]
#[ignore = "debugging"]
fn prni() {
	use crate::hook::Hook;
	use crate::{item::Item, prio::Priority};
	use env::Environment;
	use serde_yaml::Value;
	use std::path::PathBuf;
	use transform::Transformer;

	use crate::profile::Shared;

	let i = Item {
		shared: Shared {
			environment: Environment(
				[
					("Foo".into(), Value::String("Bar".into())),
					("Bool".into(), Value::Bool(true)),
				]
				.into_iter()
				.collect(),
			),
			transformers: vec![Transformer::LineTerminator(transform::LineTerminator::LF)],
			pre_hook: Some(Hook::Inline("set -eoux pipefail\necho 'Foo'".into())),
			post_hook: None,
			priority: Some(Priority(5)),
		},
		path: PathBuf::from("/dev/null"),
		rename: None,
		overwrite_target: None,
		merge: None,
	};

	serde_yaml::to_writer(std::io::stdout(), &i).unwrap();
}
