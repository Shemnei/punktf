use std::fmt;
use std::ops::{Deref, Index};

/// A general position which allows convertion from and to [`usize`] and [`u32`].
pub trait Pos {
	fn from_usize(value: usize) -> Self;
	fn from_u32(value: u32) -> Self;
	fn as_usize(&self) -> usize;
	fn as_u32(&self) -> u32;
}

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
				pub fn new(value: $inner_ty) -> Self {
					Self(value)
				}
			}

			impl Pos for $ident {
				fn from_usize(value: usize) -> Self {
					Self(value as $inner_ty)
				}

				fn from_u32(value: u32) -> Self {
					Self(value as $inner_ty)
				}

				fn as_usize(&self) -> usize {
					self.0 as usize
				}

				fn as_u32(&self) -> u32 {
					self.0 as u32
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

/// A span with a start position ([low](`ByteSpan::low`)) and an end position ([high](`ByteSpan::high`)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteSpan {
	pub low: BytePos,
	pub high: BytePos,
}

impl ByteSpan {
	pub fn new<L: Into<BytePos>, H: Into<BytePos>>(low: L, high: H) -> Self {
		let mut low = low.into();
		let mut high = high.into();

		if low > high {
			std::mem::swap(&mut low, &mut high);
		}

		Self { low, high }
	}

	pub fn span<T>(self, value: T) -> Spanned<T> {
		Spanned::new(self, value)
	}

	pub fn with_low<L: Into<BytePos>>(&self, low: L) -> Self {
		let mut copy = *self;
		copy.low = low.into();

		copy
	}

	pub fn with_high<H: Into<BytePos>>(&self, high: H) -> Self {
		let mut copy = *self;
		copy.high = high.into();

		copy
	}

	pub fn union(&self, other: &Self) -> Self {
		let low = std::cmp::min(self.low, other.low);
		let high = std::cmp::max(self.high, other.high);

		Self { low, high }
	}

	pub fn offset_low<A: Into<i32>>(&self, amount: A) -> Self {
		let amount = amount.into();

		let mut copy = *self;
		copy.low.0 = (copy.low.0 as i32 + amount) as u32;

		copy
	}

	pub fn offset_high<A: Into<i32>>(&self, amount: A) -> Self {
		let amount = amount.into();

		let mut copy = *self;
		copy.high.0 = (copy.high.0 as i32 + amount) as u32;

		copy
	}

	pub fn offset<A: Into<i32>>(&self, amount: A) -> Self {
		let amount = amount.into();

		let mut copy = *self;
		copy.low.0 = (copy.low.0 as i32 + amount) as u32;
		copy.high.0 = (copy.high.0 as i32 + amount) as u32;

		copy
	}

	pub fn low(&self) -> &BytePos {
		&self.low
	}

	pub fn high(&self) -> &BytePos {
		&self.high
	}
}

impl fmt::Display for ByteSpan {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}..{}", self.low, self.high)
	}
}

impl Index<ByteSpan> for str {
	type Output = str;

	fn index(&self, index: ByteSpan) -> &Self::Output {
		&self[index.low.as_usize()..index.high.as_usize()]
	}
}

impl Index<&ByteSpan> for str {
	type Output = str;

	fn index(&self, index: &ByteSpan) -> &Self::Output {
		&self[index.low.as_usize()..index.high.as_usize()]
	}
}

pub struct Spanned<T> {
	pub span: ByteSpan,
	pub value: T,
}

impl<T> Spanned<T> {
	pub fn new(span: ByteSpan, value: T) -> Self {
		Self { span, value }
	}

	pub fn span(&self) -> &ByteSpan {
		&self.span
	}

	pub fn value(&self) -> &T {
		&self.value
	}

	pub fn into_span(self) -> ByteSpan {
		self.span
	}

	pub fn into_value(self) -> T {
		self.value
	}

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
