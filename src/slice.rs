use std::alloc::{self, Layout};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::marker::PhantomData;
use std::mem;
use std::ops::{Deref, DerefMut};
use std::ptr::{self, NonNull};
use std::slice;

use crate::{BoxHeader, Header, TooLong};

// We want to have the null pointer optimisation but we also don't want to allocate for empty
// slices. That means we need some pointer that denotes an empty slice that we recognize and won't
// ever be returned from the allocator, but is not Null. So we simply get this pointer.
//
// Note that this may lead to unaligned pointer. That is OK if the pointer is never dereferenced.
static ZERO_SENTINEL: u8 = 0;

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
        Layout::new::<H>().extend(Layout::array::<u8>(0).unwrap()).unwrap().1
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
        unsafe {
            self.header.as_ptr().cast::<u8>().add(offset).cast::<T>()
        }
    }

    #[inline]
    fn is_sentinel(&self) -> bool {
        ptr::eq(self.header.as_ptr().cast::<u8>(), &ZERO_SENTINEL)
    }

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
        assert!(layout.size() > 0, "TODO: Handle 0 layout? Can it even happen?");
        let ptr = unsafe { alloc::alloc(layout) };
        if ptr.is_null() {
            alloc::handle_alloc_error(layout);
        }
        unsafe {
            let data_ptr = ptr.add(data_offset).cast::<T>();
            let len_ptr = ptr.add(len_off);
            let hdr = ptr.cast::<H>();

            // Initialize everything
            ptr::write(hdr, H::encode_len(len, len_ptr));
            for (idx, src) in src.iter().enumerate() {
                // FIXME: Handle panics and release the memory/call destructors. Currently it is
                // not UB, but we leak all the cloned things and the allocation. Not great.
                ptr::write(data_ptr.add(idx), src.clone());
            }

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

                alloc::dealloc(self.header.as_ptr().cast::<u8>(), layout);
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
        unsafe {
            slice::from_raw_parts(self.data(len), len)
        }
    }
}

impl<T> DerefMut for OwnedSlice<T, BoxHeader> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        if self.is_sentinel() {
            return &mut [];
        }

        let len = self.len();
        unsafe {
            slice::from_raw_parts_mut(self.data(len), len)
        }
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
{}

unsafe impl<T, H> Sync for OwnedSlice<T, H>
where
    H: Header + Send + Sync,
    T: Send + Sync,
{}

#[cfg(test)]
mod tests {
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
        let mut s = OwnedSlice::<String>::new(&[
            "Hello".to_owned(),
            "World".to_owned(),
        ]).unwrap();
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
}
