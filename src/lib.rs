//! Saving space on owned heap-allocated slices and strings.
//!
//! If we have a [`String`] containing `hello`, it takes `5` bytes on the heap and whole 24 bytes
//! on the stack (on 64bit platform). That's a lot of overhead. One can use `Box<str>` instead,
//! that uses only 16 bytes on the stack. With this library, `6` bytes are on the heap and 8 on the
//! stack (no, this is not *the* short string optimization ‒ that one stops being useful at very
//! short strings).
//!
//! Also, this library works for other arrays/slices not just strings.
//!
//! The types work with null pointer optimisation (`Option<OwnedSlice<T>>` has the same size as
//! [`OwnedSlice<T>`][OwnedSlice]) and empty slice/string doesn't allocate.
//!
//! The downside is, they can't change their length like [`String`] or [`Vec`]. Therefore, this is
//! suited for storing large amounts of smallish strings.
//!
//! # How does it work
//!
//! The length is stored as a header on the heap, followed by the actual data. The length is
//! variable length encoded ‒ short strings take only 1 byte header, longer ones take 2 bytes...
//! There's a limit at how large the string can be (current limit is 2^38 characters).
//!
//! # Future plans
//!
//! The datastructures are parametrized by a [`Header`]. The future versions will have a limited
//! [`Arc`][std::sync::Arc] or [`Rc`][std::rc::Rc] builtin functionality ‒ it'll be possible to
//! share single string/slice between multiple owners. They'll still be sized one word on the
//! stack.
//!
//! Also, there's a plan to be able to put multiple these variable length slices/strings inside a
//! single allocationd behind a single pointer. Then it'll be possible to save even more on
//! structures holding multiple shortish strings. But how the API will look like is still unknown.
//!
//! Support for integrating with other libraries (`serde`, `heapsize`) will be added behind feature
//! flags.
//!
//! Support for allocating from an arena (eg. [`bumpalo`](https://crates.io/crates/bumpalo) to cut
//! down on the allocator overhead might also come.
//!
//! # Features
//!
//! * The `std` feature (on by default) adds some little convenience details (eg. the [`TooLong`]
//!   implements [`std::error::Error`]). By opting out of this feature, the library needs only
//!   [`alloc`].
//!
//! # Current quirks
//!
//! (Some of it may be lifted in future versions)
//!
//! The structures dereference to slice/`str`, but explicit dereferencing may be necessary at
//! times.
//!
//! Sometimes it is needed to hint the type resolution with the right type (as in the example
//! below).
//!
//! # Examples
//!
//! ```rust
//! use squash::Str;
//!
//! // Takes 24 + 5 + allocator overhead
//! let string = String::from("Hello");
//! // Takes 8 + 6 + allocator overhead
//! let squashed_string: Str = Str::new(&string).unwrap();
//!
//! assert_eq!(&string as &str, &squashed_string as &str);
//! ```
//!
//! # See also
//!
//! If you are trying to save some memory, you might also have a look at these:
//!
//! * [`smallvec`](https://crates.io/crates/smallvec) and
//!   [`smallstr`](https://crates.io/crates/smallstr) (alternatively also
//!   [`tinyvec`](https://crates.io/crates/tinyvec)).
//! * [`arrayvec`](https://crates.io/crates/arrayvec) if you know an upper bound for the size.
//! * [`bumpalo`](https://crates.io/crates/bumpalo) or another arena allocator. This doesn't make
//!   the actual size smaller, though.

#![doc(test(attr(deny(warnings))))]
#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

// TODO: ArcSwap support? Is it possible?
// TODO: Serde support
// TODO: HeapSize support
// TODO: Bumpalo support
// TODO: make_mut or similar APIs?
// TODO: as_raw and similar?

mod header;
mod slice;
mod wrapper;

pub use header::boxed::BoxHeader;
pub use header::{Header, TooLong};
pub use slice::OwnedSlice;
pub use wrapper::str::Str;
