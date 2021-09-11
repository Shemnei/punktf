//! User defined variables used by [profiles](`crate::profile::Profile`) and
//! [dotfiles](`crate::Dotfile`).

use std::collections::HashMap;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

/// Variables that replace values in templates
pub trait Vars {
	/// Get a variable by name
	fn var<K: AsRef<str>>(&self, key: K) -> Option<&str>;
}

/// User defined variables
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variables {
	/// User defined variables with a name and value.
	#[serde(flatten)]
	pub inner: HashMap<String, String>,
}

impl Vars for Variables {
	fn var<K>(&self, key: K) -> Option<&str>
	where
		K: AsRef<str>,
	{
		self.inner.get(key.as_ref()).map(|value| value.deref())
	}
}

impl Variables {
	/// Creates a new instance from an iterator over key, value tuples.
	pub fn from_items<K, V, I, II>(iter: II) -> Self
	where
		K: Into<String>,
		V: Into<String>,
		I: Iterator<Item = (K, V)>,
		II: IntoIterator<IntoIter = I, Item = (K, V)>,
	{
		let inner = iter
			.into_iter()
			.map(|(k, v)| (k.into(), v.into()))
			.collect();

		Self { inner }
	}
}
