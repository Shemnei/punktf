use std::collections::HashMap;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

/// Variables that replace values it templates
pub trait Variables {
	fn var<K: AsRef<str>>(&self, key: K) -> Option<&str>;
}

/// User defined variables
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserVars {
	#[serde(flatten)]
	pub inner: HashMap<String, String>,
}

impl Variables for UserVars {
	fn var<K>(&self, key: K) -> Option<&str>
	where
		K: AsRef<str>,
	{
		self.inner.get(key.as_ref()).map(|value| value.deref())
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
}
