use std::{collections::BTreeMap, io::Write, path::Path};

use crate::env::LayeredEnvironment;

pub type Result<T, E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

pub trait TemplateEngine {
	fn render_to_write(
		&mut self,
		w: &mut dyn Write,
		name: &str,
		env: &LayeredEnvironment,
		content: &str,
	) -> Result<()>;

	fn render(&mut self, name: &str, env: &LayeredEnvironment, content: &str) -> Result<String> {
		let mut buf = Vec::new();
		self.render_to_write(&mut buf, name, env, content)?;
		Ok(String::from_utf8(buf)?)
	}
}

#[derive(Default)]
pub struct Registry(BTreeMap<&'static str, Box<dyn TemplateEngine>>);

impl Registry {
	pub fn register<E: 'static + TemplateEngine>(&mut self, extension: &'static str, engine: E) {
		self.0.insert(extension, Box::new(engine));
	}

	pub fn get_for_path(&mut self, path: &Path) -> Option<&mut dyn TemplateEngine> {
		let ext = path.extension()?.to_str()?;
		let r = self.0.get_mut(ext)?;
		Some(r.as_mut())
	}
}

pub mod mj {
	use std::io::Write;

	use crate::{env::LayeredEnvironment, value};

	use super::{Result, TemplateEngine};
	use minijinja::{value::StructObject, Environment, UndefinedBehavior, Value};

	impl From<value::Value> for Value {
		fn from(value: value::Value) -> Self {
			match value {
				value::Value::Null => Value::from(()),
				value::Value::String(v) => Value::from(v),
				value::Value::Bool(v) => Value::from(v),
				value::Value::Float(v) => Value::from(v),
				value::Value::Int(v) => Value::from(v),
			}
		}
	}

	impl StructObject for LayeredEnvironment {
		fn get_field(&self, name: &str) -> Option<Value> {
			self.get(name).map(|v| v.clone().into())
		}
	}

	pub struct MiniJinja;

	impl TemplateEngine for MiniJinja {
		fn render_to_write(
			&mut self,
			w: &mut dyn Write,
			name: &str,
			ctx: &LayeredEnvironment,
			content: &str,
		) -> Result<()> {
			let mut env = Environment::new();
			// Error on undefined variables
			env.set_undefined_behavior(UndefinedBehavior::Strict);
			env.add_template(name, content)?;

			let tmpl = env.get_template(name)?;
			let val = Value::from_struct_object(ctx.clone());
			tmpl.render_to_write(val, w)?;

			Ok(())
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn registry() {
		let mut reg = Registry::default();
		reg.register("mjinja", mj::MiniJinja);

		let eng = reg.get_for_path(Path::new("/test/path/123/file.txt.mjinja"));

		assert!(eng.is_some())
	}
}
