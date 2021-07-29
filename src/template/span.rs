use std::fmt;
use std::ops::{Deref, DerefMut, Index};

type BytePosType = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BytePos(pub BytePosType);

impl BytePos {
	pub fn new(value: BytePosType) -> Self {
		Self(value)
	}

	pub fn from_usize(value: usize) -> Self {
		Self(value as BytePosType)
	}

	pub fn as_usize(&self) -> usize {
		self.0 as usize
	}

	pub fn into_inner(self) -> BytePosType {
		self.0
	}
}

impl fmt::Display for BytePos {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Display::fmt(&self.0, f)
	}
}

impl From<usize> for BytePos {
	fn from(value: usize) -> Self {
		BytePos::from_usize(value)
	}
}

impl From<BytePosType> for BytePos {
	fn from(value: BytePosType) -> Self {
		Self(value)
	}
}

impl From<BytePos> for usize {
	fn from(value: BytePos) -> Self {
		value.as_usize()
	}
}

impl From<BytePos> for BytePosType {
	fn from(value: BytePos) -> Self {
		value.0
	}
}

impl Deref for BytePos {
	type Target = BytePosType;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for BytePos {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

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
		copy.low.0 = (copy.low.0 as i32 + amount) as BytePosType;

		copy
	}

	pub fn offset_high<A: Into<i32>>(&self, amount: A) -> Self {
		let amount = amount.into();

		let mut copy = *self;
		copy.high.0 = (copy.high.0 as i32 + amount) as BytePosType;

		copy
	}

	pub fn offset<A: Into<i32>>(&self, amount: A) -> Self {
		let amount = amount.into();

		let mut copy = *self;
		copy.low.0 = (copy.low.0 as i32 + amount) as BytePosType;
		copy.high.0 = (copy.high.0 as i32 + amount) as BytePosType;

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
