use std::str::FromStr;

use serde::Deserialize;

pub mod version {
	use serde::{de::Visitor, Deserialize, Deserializer};
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
		fn from(value: ParseIntError) -> Self {
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

	impl Version {
		pub const ZERO: Self = Version {
			major: 0,
			minor: 0,
			patch: 0,
		};

		pub fn with_major(mut self, major: u8) -> Self {
			self.major = major;
			self
		}

		pub fn with_minor(mut self, minor: u8) -> Self {
			self.minor = minor;
			self
		}

		pub fn with_patch(mut self, patch: u8) -> Self {
			self.patch = patch;
			self
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
		let eat = if !check_digit(bytes, 0) {
			// To short
			return Err(ParseVersionError::InvalidNumber);
		} else if !check_digit(bytes, 1) {
			1
		} else if !check_digit(bytes, 2) {
			2
		} else if !check_digit(bytes, 3) {
			3
		} else {
			// To long
			return Err(ParseVersionError::InvalidNumber);
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
				return Ok(Version {
					major,
					minor,
					patch,
				});
			} else {
				return Err(ParseVersionError::TrailingCharacters);
			}
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
			Ok(())
		}

		#[test]
		fn version_parse_err() -> Result<(), Box<dyn std::error::Error>> {
			assert_eq!(
				"1.".parse::<Version>(),
				Err(ParseVersionError::TrailingCharacters)
			);

			Ok(())
		}
	}
}

pub mod profile {
	use super::version;
	use std::str::FromStr;

	use serde::Deserialize;
	use thiserror::Error;

	#[derive(Debug, Error)]
	pub enum Error {
		#[error("invalid profile: {0}")]
		InvalidProfile(#[from] serde_yaml::Error),
		#[error("unsupported version: {0}")]
		UnsupportedVersion(versions::Versioning),
	}

	pub type Result<T, E = Error> = std::result::Result<T, E>;

	#[derive(Debug, Deserialize)]
	#[serde(default)]
	pub struct Version {
		version: version::Version,
	}

	impl Default for Version {
		fn default() -> Self {
			Self {
				version: version::Version::ZERO,
			}
		}
	}

	impl AsRef<version::Version> for Version {
		fn as_ref(&self) -> &version::Version {
			&self.version
		}
	}

	#[derive(Debug, Deserialize)]
	pub struct Profile {
		#[serde(flatten)]
		version: Version,
	}

	impl FromStr for Profile {
		type Err = Error;

		fn from_str(s: &str) -> Result<Self> {
			let version: Version = serde_yaml::from_str(s)?;

			println!("Read version: {version:?}");

			todo!()
		}
	}
}

#[test]
fn main() -> Result<(), Box<dyn std::error::Error>> {
	let profile = std::fs::read_to_string("profile.yaml")?;
	println!("Parsing profile:\n{profile}");
	let p = profile::Profile::from_str(&profile)?;

	Ok(())
}
