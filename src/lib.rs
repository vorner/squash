// TODO: Can we make this alloc-only?

mod header;
mod slice;
mod wrapper;

pub use header::boxed::BoxHeader;
pub use header::{Header, TooLong};
pub use slice::OwnedSlice;
pub use wrapper::str::Str;
