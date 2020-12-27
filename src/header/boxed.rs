use super::{Header, TooLong};

pub struct BoxHeader(u8);

// TODO: Variable length encoding
unsafe impl Header for BoxHeader {
    #[inline]
    fn extra_needed(len: usize) -> Result<usize, TooLong> {
        if len > u8::MAX as usize {
            Err(TooLong)
        } else {
            Ok(0)
        }
    }
    #[inline]
    unsafe fn encode_len(len: usize, _: *mut u8) -> Self {
        assert!(len <= u8::MAX as usize);
        Self(len as u8)
    }
    #[inline]
    unsafe fn decode_len(&self, _: *const u8) -> usize {
        self.0 as usize
    }
    #[inline]
    fn inc(&self) -> bool {
        false
    }
    #[inline]
    fn dec(&self) -> bool {
        true
    }
}
