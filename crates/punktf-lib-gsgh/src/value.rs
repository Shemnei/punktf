#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Value {
	Null,
	String(String),
	Bool(bool),
	Float(f64),
	Int(i64),
}

impl From<String> for Value {
	fn from(value: String) -> Self {
		Self::String(value)
	}
}

impl From<&str> for Value {
	fn from(value: &str) -> Self {
		Self::String(value.to_owned())
	}
}

impl From<bool> for Value {
	fn from(value: bool) -> Self {
		Self::Bool(value)
	}
}

impl From<&bool> for Value {
	fn from(value: &bool) -> Self {
		Self::Bool(*value)
	}
}

impl<T: Into<Value>> From<Option<T>> for Value {
	fn from(value: Option<T>) -> Self {
		let Some(v) = value else {
			return Value::Null;
		};
		v.into()
	}
}

impl From<()> for Value {
	fn from(_: ()) -> Self {
		Self::Null
	}
}

pub mod ser {
	use super::Value;
	use serde::ser;

	impl ser::Serialize for Value {
		fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
		where
			S: serde::Serializer,
		{
			match self {
				Value::Null => serializer.serialize_unit(),
				Value::String(v) => serializer.serialize_str(v),
				Value::Bool(v) => serializer.serialize_bool(*v),
				Value::Float(v) => serializer.serialize_f64(*v),
				Value::Int(v) => serializer.serialize_i64(*v),
			}
		}
	}
}

pub mod de {
	use serde::de;

	use super::Value;

	impl<'de> de::Deserialize<'de> for Value {
		fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
		where
			D: serde::Deserializer<'de>,
		{
			struct ValueVisitor;

			impl<'de> de::Visitor<'de> for ValueVisitor {
				type Value = Value;

				fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
					formatter.write_str("any literal value")
				}

				fn visit_i8<E>(self, v: i8) -> Result<Self::Value, E>
				where
					E: de::Error,
				{
					Ok(Value::Int(v as i64))
				}

				fn visit_i16<E>(self, v: i16) -> Result<Self::Value, E>
				where
					E: de::Error,
				{
					Ok(Value::Int(v as i64))
				}

				fn visit_i32<E>(self, v: i32) -> Result<Self::Value, E>
				where
					E: de::Error,
				{
					Ok(Value::Int(v as i64))
				}

				fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
				where
					E: de::Error,
				{
					Ok(Value::Int(v as i64))
				}

				fn visit_u8<E>(self, v: u8) -> Result<Self::Value, E>
				where
					E: de::Error,
				{
					Ok(Value::Int(v as i64))
				}

				fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
				where
					E: de::Error,
				{
					Ok(Value::Int(v as i64))
				}

				fn visit_u32<E>(self, v: u32) -> Result<Self::Value, E>
				where
					E: de::Error,
				{
					Ok(Value::Int(v as i64))
				}

				fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
				where
					E: de::Error,
				{
					Ok(Value::Int(v as i64))
				}

				fn visit_f32<E>(self, v: f32) -> Result<Self::Value, E>
				where
					E: de::Error,
				{
					Ok(Value::Float(v as f64))
				}

				fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
				where
					E: de::Error,
				{
					Ok(Value::Float(v as f64))
				}

				fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
				where
					E: de::Error,
				{
					Ok(Value::String(v.to_owned()))
				}

				fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
				where
					E: de::Error,
				{
					Ok(Value::String(v))
				}

				fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
				where
					E: de::Error,
				{
					Ok(Value::Bool(v))
				}

				fn visit_none<E>(self) -> Result<Self::Value, E>
				where
					E: de::Error,
				{
					Ok(Value::Null)
				}

				fn visit_unit<E>(self) -> Result<Self::Value, E>
				where
					E: de::Error,
				{
					Ok(Value::Null)
				}
			}

			deserializer.deserialize_any(ValueVisitor)
		}
	}
}
