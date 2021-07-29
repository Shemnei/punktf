use std::fmt;
use std::ops::{Deref, DerefMut, Index};

type CharPosType = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CharPos(pub CharPosType);

impl CharPos {
	pub fn new(value: CharPosType) -> Self {
		Self(value)
	}

	pub fn from_usize(value: usize) -> Self {
		Self(value as CharPosType)
	}

	pub fn as_usize(&self) -> usize {
		self.0 as usize
	}

	pub fn into_inner(self) -> CharPosType {
		self.0
	}
}

impl fmt::Display for CharPos {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		fmt::Display::fmt(&self.0, f)
	}
}

impl From<usize> for CharPos {
	fn from(value: usize) -> Self {
		CharPos::from_usize(value)
	}
}

impl From<CharPosType> for CharPos {
	fn from(value: CharPosType) -> Self {
		Self(value)
	}
}

impl From<CharPos> for usize {
	fn from(value: CharPos) -> Self {
		value.as_usize()
	}
}

impl From<CharPos> for CharPosType {
	fn from(value: CharPos) -> Self {
		value.0
	}
}

impl Deref for CharPos {
	type Target = CharPosType;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for CharPos {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CharSpan {
	pub low: CharPos,
	pub high: CharPos,
}

impl CharSpan {
	pub fn new<L: Into<CharPos>, H: Into<CharPos>>(low: L, high: H) -> Self {
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

	pub fn with_low<L: Into<CharPos>>(&self, low: L) -> Self {
		let mut copy = *self;
		copy.low = low.into();

		copy
	}

	pub fn with_high<H: Into<CharPos>>(&self, high: H) -> Self {
		let mut copy = *self;
		copy.high = high.into();

		copy
	}

	pub fn union(&self, other: &Self) -> Self {
		let low = std::cmp::min(self.low, other.low);
		let high = std::cmp::max(self.high, other.high);

		Self { low, high }
	}

	pub fn offset_low(&self, amount: i32) -> Self {
		let mut copy = *self;
		// TODO: make better
		copy.low.0 = (copy.low.0 as i32 + amount) as CharPosType;

		copy
	}

	pub fn offset_high(&self, amount: i32) -> Self {
		let mut copy = *self;
		// TODO: make better
		copy.high.0 = (copy.high.0 as i32 + amount) as CharPosType;

		copy
	}

	pub fn offset(&self, amount: i32) -> Self {
		let mut copy = *self;
		copy.low.0 = (copy.low.0 as i32 + amount) as CharPosType;
		copy.high.0 = (copy.high.0 as i32 + amount) as CharPosType;

		copy
	}

	pub fn low(&self) -> &CharPos {
		&self.low
	}

	pub fn high(&self) -> &CharPos {
		&self.high
	}
}

impl fmt::Display for CharSpan {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}..{}", self.low, self.high)
	}
}

impl Index<CharSpan> for str {
	type Output = str;

	fn index(&self, index: CharSpan) -> &Self::Output {
		&self[index.low.as_usize()..index.high.as_usize()]
	}
}

impl Index<&CharSpan> for str {
	type Output = str;

	fn index(&self, index: &CharSpan) -> &Self::Output {
		&self[index.low.as_usize()..index.high.as_usize()]
	}
}

pub struct Spanned<T> {
	pub span: CharSpan,
	pub value: T,
}

impl<T> Spanned<T> {
	pub fn new(span: CharSpan, value: T) -> Self {
		Self { span, value }
	}

	pub fn span(&self) -> &CharSpan {
		&self.span
	}

	pub fn value(&self) -> &T {
		&self.value
	}

	pub fn into_span(self) -> CharSpan {
		self.span
	}

	pub fn into_value(self) -> T {
		self.value
	}

	pub fn into_inner(self) -> (CharSpan, T) {
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
