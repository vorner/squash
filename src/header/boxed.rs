use core::convert::TryInto;
use core::ptr;

use super::{Header, TooLong};

const EXTRA_MASK: u8 = 0b11;
const INLINE_BITS: u32 = 6;
const INLINE_MASK: u8 = 0b111111;
const MAX_EXTRAS: usize = 4;

/// A header without sharing support.
///
/// The data will be uniquely owned. Lengths up to 64 are encoded inline in this header. It is
/// possible to get a mutable access to the elements of the slice.
///
/// This is the default [`Header`] implementation if none is set.
pub struct BoxHeader(u8);

unsafe impl Header for BoxHeader {
    #[inline]
    fn extra_needed(len: usize) -> Result<usize, TooLong> {
        let len: u64 = len.try_into().map_err(|_| TooLong)?;
        let zeroes = len.leading_zeros();
        let significant = 64 - zeroes;
        // We store 6 bits inline in ourselves, then can have up to 4 extra bytes for length.
        let extra = ((significant.saturating_sub(INLINE_BITS)) + 7) / 8;
        let extra = extra as usize;

        if extra <= MAX_EXTRAS {
            Ok(extra)
        } else {
            Err(TooLong)
        }
    }
    #[inline]
    unsafe fn encode_len(len: usize, extra: *mut u8) -> Self {
        let extra_len = Self::extra_needed(len).unwrap();
        let len = len as u64;
        let bytes = len.to_le_bytes();

        ptr::copy_nonoverlapping(bytes.as_ptr(), extra, extra_len);

        let encoded =
            ((extra_len as u8 & EXTRA_MASK) << INLINE_BITS) | (bytes[extra_len] & INLINE_MASK);
        Self(encoded)
    }
    #[inline]
    unsafe fn decode_len(&self, extra: *const u8) -> usize {
        let extra_len = self.0 >> INLINE_BITS;
        let mut buf = [0; 8];
        ptr::copy_nonoverlapping(extra, buf.as_mut_ptr(), extra_len as usize);
        buf[extra_len as usize] = self.0 & INLINE_MASK;
        let len = u64::from_le_bytes(buf);
        len as usize
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

#[cfg(all(feature = "std", test))]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn zero() {
        assert_eq!(0, BoxHeader::extra_needed(0).unwrap());
        let mut buf = [];
        unsafe {
            let h = BoxHeader::encode_len(0, buf.as_mut_ptr());
            assert_eq!(0, h.0);
            assert_eq!(0, h.decode_len(buf.as_ptr()));
        }
    }

    #[test]
    fn small() {
        assert_eq!(0, BoxHeader::extra_needed(50).unwrap());
        let mut buf = [];
        unsafe {
            let h = BoxHeader::encode_len(50, buf.as_mut_ptr());
            assert_eq!(50, h.0);
            assert_eq!(50, h.decode_len(buf.as_ptr()));
        }
    }

    #[test]
    fn large() {
        assert_eq!(1, BoxHeader::extra_needed(350).unwrap());
        let mut buf = [0];
        unsafe {
            let h = BoxHeader::encode_len(350, buf.as_mut_ptr());
            assert_eq!(0b0100_0001, h.0);
            assert_eq!(94, buf[0]);
            assert_eq!(350, h.decode_len(buf.as_ptr()));
        }
    }

    proptest! {
        #[test]
        fn random_len(len: usize) {
            if let Ok(extra) = BoxHeader::extra_needed(len) {
                prop_assert!(extra <= 4);
                let mut buf = vec![0; extra];
                // make sure there's no extra space and any kind of overflow would get detected
                buf.shrink_to_fit();
                unsafe {
                    let h = BoxHeader::encode_len(len, buf.as_mut_ptr());
                    prop_assert_eq!(len, h.decode_len(buf.as_ptr()));
                }
            }
        }
    }
}
