use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

pub(crate) mod boxed;

/// An error returned when the slice or string is longer than the header is able to encode.
///
/// The headers have a limit of how large lengths they can encode which is usually smaller than
/// what the whole `usize` can hold (at least on 64bit platforms). If they are asked to encode
/// something larger, this error is returned.
///
/// Note that the limits are usually above practical usability limits and if strings of over 4GB
/// are actually needed, the usefulness of this library is questionable (it optimizes for many
/// small strings/slices, the overhead is negligible on these large behemoths).
#[derive(Copy, Clone, Debug)]
pub struct TooLong;

impl Display for TooLong {
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
        write!(fmt, "Too long")
    }
}

impl Error for TooLong {}

/// Description of the header encoding a length.
///
/// This is responsible to hold both a reference count (if applicable) and the length of the slice.
///
/// Note that it is not a trait consumers of this library would use directly and most common
/// implementations should be provided, so it is unlikely one would need to interact with it,
/// except for picking the right one as the type parameter of the [`OwnedSlice`][crate::OwnedSlice]
/// or similar type.
///
/// # Safety
///
/// The trait must correctly decode the same length as was encoded.
///
/// The reference counting must properly "pair" ‒ it must not ask for destruction while someone
/// still holds a reference count.
pub unsafe trait Header {
    /// How many extra bytes are needed for encoding this length.
    ///
    /// Returns the amount of bytes needed, or signals that the length is too long for encoding.
    fn extra_needed(len: usize) -> Result<usize, TooLong>;

    /// Creates a new header and encodes the length.
    ///
    /// Will be called with as many bytes as the [`extra_needed`][Header::extra_needed] designated,
    /// passed as a pointer in the `extra` parameter.
    ///
    /// It shall encode a reference count of 1.
    ///
    /// # Safety
    ///
    /// The `extra` must point to at least as many bytes as asked for by
    /// [`extra_needed`][Header::extra_needed].
    unsafe fn encode_len(len: usize, extra: *mut u8) -> Self;

    /// Decodes the previously encoded length.
    ///
    /// The extra bytes are provided back to it. Note that it is up to the header to know how many
    /// bytes there are.
    ///
    /// # Safety
    ///
    /// The `extra` must point to the bytes previously passed to
    /// [`encode_len`][Header::encode_len].
    unsafe fn decode_len(&self, extra: *const u8) -> usize;

    /// Increment the reference count.
    ///
    /// Returns a success flag. If the reference count exceeds what the header can hold, a false is
    /// returned to signal that it was *not* incremented. In that case, the
    /// [`OwnedSlice`][crate::OwnedSlice] gets fully cloned instead.
    fn inc(&self) -> bool;

    /// Decrements a reference count.
    ///
    /// Returns if the reference count dropped to 0 and the slice should be destroyed.
    fn dec(&self) -> bool;
}
