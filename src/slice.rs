use alloc::alloc::{alloc as mem_alloc, dealloc as mem_dealloc, handle_alloc_error, Layout};
use alloc::fmt::{Debug, Formatter, Result as FmtResult};
use core::cell::Cell;
use core::marker::PhantomData;
use core::mem;
use core::ops::{Deref, DerefMut};
use core::ptr::{self, NonNull};
use core::slice;

use crate::{BoxHeader, Header, TooLong};

// We want to have the null pointer optimisation but we also don't want to allocate for empty
// slices. That means we need some pointer that denotes an empty slice that we recognize and won't
// ever be returned from the allocator, but is not Null. So we simply get this pointer.
//
// Note that this may lead to unaligned pointer. That is OK if the pointer is never dereferenced.
static ZERO_SENTINEL: u8 = 0;

/// An owned slice.
///
/// This is similar to `Box<[T]>` (or `Arc<[T]>`, depending on the [`Header`] `H` type parameter).
/// It holds a heap allocated slice of fixed length. The difference is in internal representation ‒
/// this is behind a thin pointer and encoded with smaller memory overhead (small slices don't need
/// full 8 bytes of length).
///
/// # Examples
///
/// ```rust
/// use squash::OwnedSlice;
/// let s: OwnedSlice<u16> = OwnedSlice::new(&[1, 2, 3]).unwrap();
/// assert_eq!(3, s.len());
/// ```
///
/// # Internal representation
///
/// The heap layout is the header, followed by exactly the number of extra bytes the header needed
/// to encode the length, followed by the actual slice data, with alignments taken into account.
pub struct OwnedSlice<T, H = BoxHeader>
where
    H: Header,
{
    header: NonNull<H>,
    _data: PhantomData<T>,
}

impl<T, H> OwnedSlice<T, H>
where
    H: Header,
{
    #[inline]
    fn len(&self) -> usize {
        if self.is_sentinel() {
            return 0;
        }

        unsafe {
            let header = &*self.header.as_ref();
            let len_data = self.header.as_ptr().cast::<u8>().add(Self::len_offset());
            header.decode_len(len_data)
        }
    }

    #[inline]
    fn len_offset() -> usize {
        Layout::new::<H>()
            .extend(Layout::array::<u8>(0).unwrap())
            .unwrap()
            .1
    }

    #[inline]
    fn layout_and_offsets(len: usize) -> Result<(Layout, usize, usize), TooLong> {
        let extra = H::extra_needed(len)?;
        let (l1, len_off) = Layout::new::<H>()
            .extend(Layout::array::<u8>(extra).expect("Insanely large stuff"))
            .expect("Insanely large stuff");
        let data_layout = Layout::array::<T>(len).expect("Insanely large stuff");
        let (layout, data_off) = l1.extend(data_layout).expect("Insanely large stuff");
        Ok((layout, len_off, data_off))
    }

    #[inline]
    fn layout(len: usize) -> Layout {
        Self::layout_and_offsets(len).unwrap().0
    }

    #[inline]
    fn data_offset(len: usize) -> usize {
        Self::layout_and_offsets(len).unwrap().2
    }

    #[inline]
    fn data(&self, len: usize) -> *mut T {
        let offset = Self::data_offset(len);
        unsafe { self.header.as_ptr().cast::<u8>().add(offset).cast::<T>() }
    }

    #[inline]
    fn is_sentinel(&self) -> bool {
        ptr::eq(self.header.as_ptr().cast::<u8>(), &ZERO_SENTINEL)
    }

    /// Creates a new owned slice by cloning a content of the passed one.
    ///
    /// # Errors
    ///
    /// If the slice is bigger than the header can encode, this is signalized by the [`TooLong`]
    /// error. Note that the limits of headers provided by this library are generally quite
    /// generous and many users may opt to handle the theoretical errors by unwrapping/panicking.
    pub fn new(src: &[T]) -> Result<Self, TooLong>
    where
        T: Clone,
    {
        if src.is_empty() {
            // Use the sentinel thing
            return Ok(Self::default());
        }

        let len = src.len();
        let (layout, len_off, data_offset) = Self::layout_and_offsets(len)?;
        assert!(
            layout.size() > 0,
            "TODO: Handle 0 layout? Can it even happen?"
        );
        unsafe {
            let ptr = mem_alloc(layout);
            if ptr.is_null() {
                handle_alloc_error(layout);
            }

            let data_ptr = ptr.add(data_offset).cast::<T>();
            let len_ptr = ptr.add(len_off);
            let hdr = ptr.cast::<H>();

            // Initialize everything
            ptr::write(hdr, H::encode_len(len, len_ptr));
            let initialized = Cell::new(0);

            // Deal with possibly panicking during the initialization (clone is about the only
            // place where it can panic).
            struct CleanupGuard<'a, T> {
                initialized: &'a Cell<usize>,
                data_ptr: *mut T,
                ptr: *mut u8,
                layout: Layout,
            }
            impl<T> Drop for CleanupGuard<'_, T> {
                fn drop(&mut self) {
                    unsafe {
                        for i in 0..=self.initialized.get() {
                            ptr::drop_in_place(self.data_ptr.add(i));
                        }
                        mem_dealloc(self.ptr, self.layout);
                    }
                }
            }
            let guard = CleanupGuard {
                initialized: &initialized,
                data_ptr,
                ptr,
                layout,
            };

            for (idx, src) in src.iter().enumerate() {
                ptr::write(data_ptr.add(idx), src.clone());
                initialized.set(idx);
            }

            // Confirm we are done and disarm the guard (it contains no allocation, so this doesn't
            // leak).
            mem::forget(guard);

            Ok(Self {
                header: NonNull::new(hdr).unwrap(),
                _data: PhantomData,
            })
        }
    }

    // TODO: Some more constructors? Something without cloning?
}

impl<T, H> Drop for OwnedSlice<T, H>
where
    H: Header,
{
    fn drop(&mut self) {
        if self.is_sentinel() {
            return;
        }

        unsafe {
            if self.header.as_ref().dec() {
                let len = self.len();
                let layout = Self::layout(len);
                if mem::needs_drop::<T>() {
                    let data = self.data(len);

                    for i in 0..len {
                        ptr::drop_in_place(data.add(i));
                    }
                }

                mem_dealloc(self.header.as_ptr().cast::<u8>(), layout);
            }
        }
    }
}

impl<T, H> Clone for OwnedSlice<T, H>
where
    H: Header,
    T: Clone,
{
    fn clone(&self) -> Self {
        if !self.is_sentinel() && unsafe { self.header.as_ref().inc() } {
            Self {
                header: self.header,
                _data: PhantomData,
            }
        } else {
            Self::new(self.deref()).expect("Already have layout for this size")
        }
    }
}

impl<T, H> Deref for OwnedSlice<T, H>
where
    H: Header,
{
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        if self.is_sentinel() {
            return &[];
        }

        let len = self.len();
        unsafe { slice::from_raw_parts(self.data(len), len) }
    }
}

impl<T> DerefMut for OwnedSlice<T, BoxHeader> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        if self.is_sentinel() {
            return &mut [];
        }

        let len = self.len();
        unsafe { slice::from_raw_parts_mut(self.data(len), len) }
    }
}

impl<T, H> Debug for OwnedSlice<T, H>
where
    H: Header,
    T: Debug,
{
    fn fmt(&self, fmt: &mut Formatter) -> FmtResult {
        self.deref().fmt(fmt)
    }
}

impl<T, H> Default for OwnedSlice<T, H>
where
    H: Header,
{
    fn default() -> Self {
        Self {
            header: NonNull::new((&ZERO_SENTINEL as *const u8 as *mut u8).cast()).unwrap(),
            _data: PhantomData,
        }
    }
}

// With some headers, we do Arc-like sharing of stuff. Therefore we need to be conservative about
// these and require both Send + Sync as the bounds, just like Arc.
unsafe impl<T, H> Send for OwnedSlice<T, H>
where
    H: Header + Send + Sync,
    T: Send + Sync,
{
}

unsafe impl<T, H> Sync for OwnedSlice<T, H>
where
    H: Header + Send + Sync,
    T: Send + Sync,
{
}

#[cfg(all(feature = "std", test))]
mod tests {
    use std::panic;

    use super::*;

    /// Check we have the null-pointer optimisation.
    #[test]
    fn null_ptr_opt() {
        assert_eq!(
            mem::size_of::<OwnedSlice<String>>(),
            mem::size_of::<Option<OwnedSlice<String>>>(),
        );
    }

    /// Exercise the special handling of the sentinel.
    #[test]
    fn empty() {
        let mut s = OwnedSlice::<String>::new(&[]).unwrap();
        assert_eq!(s.deref(), &[] as &[String]);
        assert_eq!(s.deref_mut(), &mut [] as &mut [String]);
        let s2 = s.clone();
        assert_eq!(&s as &[_], &s2 as &[_]);
        assert_eq!("[]", format!("{:?}", s));

        let s3 = OwnedSlice::<String>::default();
        assert_eq!(&s as &[_], &s3 as &[_]);
    }

    /// Test with few strings.
    ///
    /// Use strings so miri can check we run destructors alright.
    #[test]
    fn full() {
        let mut s = OwnedSlice::<String>::new(&["Hello".to_owned(), "World".to_owned()]).unwrap();
        assert_eq!(2, s.len());
        assert_eq!(s[1], "World");
        s[0] = "Round".to_owned();
        assert_eq!(s[0], "Round");
        let s2 = s.clone();
        assert_eq!(s.deref(), s2.deref());
        assert_eq!(2, s2.len());
        assert_eq!("[\"Round\", \"World\"]", format!("{:?}", s2));
    }

    #[test]
    fn long() {
        let long = vec![0u8; 300];
        let s = OwnedSlice::<_>::new(&long).unwrap();
        assert_eq!(long.deref(), s.deref());
    }

    /// Check we can handle panics during partial initialization.
    ///
    /// Miri will catch anything we might forget to deallocate. Therefore we put strings in there
    /// just to make sure there's some allocation in there.
    #[test]
    fn panic_in_init() {
        struct MaybePanic(String);

        impl Clone for MaybePanic {
            fn clone(&self) -> Self {
                if self.0 == "!!!" {
                    panic!("Panicking for the good measure of it");
                } else {
                    Self(self.0.clone())
                }
            }
        }

        let input = vec![
            MaybePanic("One".to_owned()),
            MaybePanic("Two".to_owned()),
            MaybePanic("!!!".to_owned()),
            MaybePanic("Three".to_owned()),
        ];

        panic::catch_unwind(|| {
            let _ = OwnedSlice::<MaybePanic>::new(&input);
        })
        .unwrap_err();
    }
}
