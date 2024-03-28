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
			return Ok(Version {
				major,
				..Default::default()
			});
		};

		let Some((s, minor)) = parse_u8(s)? else {
			// The parse `.` is trailing
			return Err(ParseVersionError::TrailingCharacters);
		};

		let Some(s) = parse_dot(s)? else {
			return Ok(Version {
				major,
				minor,
				..Default::default()
			});
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
