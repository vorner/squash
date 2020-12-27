use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

pub(crate) mod boxed;

#[derive(Copy, Clone, Debug)]
pub struct TooLong;

impl Display for TooLong {
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
        write!(fmt, "Too long")
    }
}

impl Error for TooLong {}

pub unsafe trait Header {
    fn extra_needed(len: usize) -> Result<usize, TooLong>;
    unsafe fn encode_len(len: usize, extra: *mut u8) -> Self;
    unsafe fn decode_len(&self, extra: *const u8) -> usize;
    fn inc(&self) -> bool;
    fn dec(&self) -> bool;
}
