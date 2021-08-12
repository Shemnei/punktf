use std::borrow::Cow;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Variables that replace values it templates
pub trait Variables {
	fn var<K: AsRef<str>>(&self, key: K) -> Option<Cow<'_, String>>;
}

/// User defined variables
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserVars {
	#[serde(flatten)]
	pub inner: HashMap<String, String>,
}

impl Variables for UserVars {
	fn var<K>(&self, key: K) -> Option<Cow<'_, String>>
	where
		K: AsRef<str>,
	{
		self.inner.get(key.as_ref()).map(Cow::Borrowed)
	}
}

impl UserVars {
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

	/// Merges everything from `other` into `self`.
	/// Fields from `self` have precendence over `other`.
	pub fn merge(&mut self, other: Self) {
		self.inner.extend(
			other
				.inner
				.into_iter()
				.filter(|(key, _)| !self.inner.contains_key(key))
				.collect::<HashMap<_, _>>(),
		);
	}
}

/// Variables whose values come from the systems environment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SystemEnv;

impl Variables for SystemEnv {
	fn var<K>(&self, key: K) -> Option<Cow<'_, String>>
	where
		K: AsRef<str>,
	{
		std::env::var(key.as_ref()).ok().map(Cow::Owned)
	}
}
