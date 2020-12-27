use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::ops::{Deref, DerefMut};
use std::str;

use crate::{BoxHeader, Header, OwnedSlice, TooLong};

#[derive(Clone, Default)]
pub struct Str<H: Header = BoxHeader>(OwnedSlice<u8, H>);

impl<H> Str<H>
where
    H: Header,
{
    pub fn new(s: &str) -> Result<Self, TooLong> {
        OwnedSlice::new(s.as_bytes()).map(Self)
    }
}

impl<H> Debug for Str<H>
where
    H: Header,
{
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
        write!(fmt, "{:?}", self.deref())
    }
}

impl<H> Display for Str<H>
where
    H: Header,
{
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
        write!(fmt, "{}", self.deref())
    }
}

impl<H> Deref for Str<H>
where
    H: Header,
{
    type Target = str;

    fn deref(&self) -> &str {
        // It was created from str originally
        unsafe { str::from_utf8_unchecked(&self.0) }
    }
}

impl DerefMut for Str<BoxHeader> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // It was created from str originally
        unsafe { str::from_utf8_unchecked_mut(&mut self.0) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings() {
        let s: Str = Str::new("Hello").unwrap();
        assert_eq!("Hello", s.deref());
        assert_eq!("Hello", s.to_string());
        assert_eq!("\"Hello\"", format!("{:?}", s));
    }
}
