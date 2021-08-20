//! Everting needed to track a position in a [source](`super::source::Source`)
//! file.

use std::fmt;
use std::ops::{Deref, Index};

// COPYRIGHT by Rust project contributors
// <https://github.com/rust-lang/rust/graphs/contributors>
//
// Copied from <https://github.com/rust-lang/rust/blob/362e0f55eb1f36d279e5c4a58fb0fe5f9a2c579d/compiler/rustc_span/src/lib.rs#L1768>.
/// A general position which allows convertion from and to [`usize`] and [`u32`].
pub trait Pos {
	/// Creates a new position from `value`.
	fn from_usize(value: usize) -> Self;

	/// Creates a new position from `value`.
	fn from_u32(value: u32) -> Self;

	/// Interprets the position as a `usize`.
	fn as_usize(&self) -> usize;

	/// Interprets the position as a `u32`.
	fn as_u32(&self) -> u32;
}

// COPYRIGHT by Rust project contributors
// <https://github.com/rust-lang/rust/graphs/contributors>
//
// Copied from <https://github.com/rust-lang/rust/blob/362e0f55eb1f36d279e5c4a58fb0fe5f9a2c579d/compiler/rustc_span/src/lib.rs#L1775> with slight adaptations.
macro_rules! pos {
    (
        $(
            $(#[$attr:meta])*
            $vis:vis struct $ident:ident($inner_vis:vis $inner_ty:ty);
        )*
    ) => {
        $(
            $(#[$attr])*
            $vis struct $ident($inner_vis $inner_ty);

			impl $ident {
				/// Creates a new instance with `value`.
				pub const fn new(value: $inner_ty) -> Self {
					Self(value)
				}
			}

			impl Pos for $ident {
				fn from_usize(value: usize) -> Self {
					Self(value as $inner_ty)
				}

				fn from_u32(value: u32) -> Self {
					Self(value)
				}

				fn as_usize(&self) -> usize {
					self.0 as usize
				}

				fn as_u32(&self) -> u32 {
					self.0
				}
			}

			impl ::std::convert::From<usize> for $ident {
				fn from(value: usize) -> Self {
					Self::from_usize(value)
				}
			}

			impl ::std::convert::From<u32> for $ident {
				fn from(value: u32) -> Self {
					Self::from_u32(value)
				}
			}

			impl ::std::fmt::Display for $ident {
				fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
					::std::fmt::Display::fmt(&self.0, f)
				}
			}
		)*
	}
}

pos! {
	/// A position of a byte.
	#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
	pub struct BytePos(pub u32);

	/// A position of a character.
	#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
	pub struct CharPos(pub u32);
}

// COPYRIGHT by Rust project contributors
// <https://github.com/rust-lang/rust/graphs/contributors>
//
// Inspired by <https://github.com/rust-lang/rust/blob/362e0f55eb1f36d279e5c4a58fb0fe5f9a2c579d/compiler/rustc_span/src/lib.rs#L419>.
/// A span with a [start position](`ByteSpan::low`) and an [end position](`ByteSpan::high`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteSpan {
	/// Start position of the span.
	pub low: BytePos,

	/// End position of the span.
	pub high: BytePos,
}

impl ByteSpan {
	/// Creates a new span from `low` and `high`.
	///
	/// # Note
	///
	/// If `high` is smaller than `low` the values are switched.
	pub fn new<L: Into<BytePos>, H: Into<BytePos>>(low: L, high: H) -> Self {
		let mut low = low.into();
		let mut high = high.into();

		if low > high {
			std::mem::swap(&mut low, &mut high);
		}

		Self { low, high }
	}

	/// Associates the span with the given `value`.
	pub const fn span<T>(self, value: T) -> Spanned<T> {
		Spanned::new(self, value)
	}

	/// Creates a new span with `low` while `high` is taken from this span.
	pub fn with_low<L: Into<BytePos>>(&self, low: L) -> Self {
		let mut copy = *self;
		copy.low = low.into();

		copy
	}

	/// Creates a new span with `low` taken from this span and `high`.
	pub fn with_high<H: Into<BytePos>>(&self, high: H) -> Self {
		let mut copy = *self;
		copy.high = high.into();

		copy
	}

	/// Creates a new span containing both `self` and `other`.
	pub fn union(&self, other: &Self) -> Self {
		let low = std::cmp::min(self.low, other.low);
		let high = std::cmp::max(self.high, other.high);

		Self { low, high }
	}

	/// Creates a new span with `low` offset by `amount`.
	pub fn offset_low<A: Into<i32>>(&self, amount: A) -> Self {
		let amount = amount.into();

		let mut copy = *self;
		copy.low.0 = (copy.low.0 as i32 + amount) as u32;

		copy
	}

	/// Creates a new span with `high` offset by `amount`.
	pub fn offset_high<A: Into<i32>>(&self, amount: A) -> Self {
		let amount = amount.into();

		let mut copy = *self;
		copy.high.0 = (copy.high.0 as i32 + amount) as u32;

		copy
	}

	/// Creates a new span with both `low` and `high` offset by `amount`.
	pub fn offset<A: Into<i32>>(&self, amount: A) -> Self {
		let amount = amount.into();

		let mut copy = *self;
		copy.low.0 = (copy.low.0 as i32 + amount) as u32;
		copy.high.0 = (copy.high.0 as i32 + amount) as u32;

		copy
	}

	/// Returns the start of the span.
	pub const fn low(&self) -> &BytePos {
		&self.low
	}

	/// Returns the end of the span.
	pub const fn high(&self) -> &BytePos {
		&self.high
	}
}

impl fmt::Display for ByteSpan {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}..{}", self.low, self.high)
	}
}

impl Index<ByteSpan> for str {
	type Output = Self;

	fn index(&self, index: ByteSpan) -> &Self::Output {
		&self[index.low.as_usize()..index.high.as_usize()]
	}
}

impl Index<&ByteSpan> for str {
	type Output = Self;

	fn index(&self, index: &ByteSpan) -> &Self::Output {
		&self[index.low.as_usize()..index.high.as_usize()]
	}
}

/// Associates a [`ByteSpan`] with a generic `value`.
pub struct Spanned<T> {
	/// A span.
	pub span: ByteSpan,

	/// The value associated with the span.
	pub value: T,
}

impl<T> Spanned<T> {
	/// Creates a new instance.
	pub const fn new(span: ByteSpan, value: T) -> Self {
		Self { span, value }
	}

	/// Returns the `span` associated with this struct.
	pub const fn span(&self) -> &ByteSpan {
		&self.span
	}

	/// Returns the `value` associated with this struct.
	pub const fn value(&self) -> &T {
		&self.value
	}

	/// Consumes self and returns the `span` associated with it.
	// Destructors can not be run at compile time.
	#[allow(clippy::missing_const_for_fn)]
	pub fn into_span(self) -> ByteSpan {
		self.span
	}

	/// Consumes self and returns the `value` associated with it.
	// Destructors can not be run at compile time.
	#[allow(clippy::missing_const_for_fn)]
	pub fn into_value(self) -> T {
		self.value
	}

	/// Consumes self and returns both `span` and `value`.
	// Destructors can not be run at compile time.
	#[allow(clippy::missing_const_for_fn)]
	pub fn into_inner(self) -> (ByteSpan, T) {
		(self.span, self.value)
	}
}

impl<T> fmt::Debug for Spanned<T>
where
	T: fmt::Debug,
{
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Spanned")
			.field("span", &self.span)
			.field("value", &self.value)
			.finish()
	}
}

impl<T> Clone for Spanned<T>
where
	T: Clone,
{
	fn clone(&self) -> Self {
		Self {
			span: self.span,
			value: self.value.clone(),
		}
	}
}

impl<T> Copy for Spanned<T> where T: Copy {}

impl<T> PartialEq for Spanned<T>
where
	T: PartialEq,
{
	fn eq(&self, other: &Self) -> bool {
		self.span.eq(&other.span) && self.value.eq(&other.value)
	}
}

impl<T> Eq for Spanned<T> where T: Eq {}

impl<T> Deref for Spanned<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.value
	}
}
