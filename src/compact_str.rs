use std::{cmp, mem, ptr, slice};

#[repr(C, align(8))]
pub struct CompactString<const N: usize> {
    bytes: [u8; N],
}

impl<const N: usize> CompactString<N> {
    // Where to store the string depends on last byte:
    // 1. 0..192: on stack, len = N
    // 2. 192..253: on stack, len = last_byte - 192
    // 3. 253: on heap, ptr = first usize, len = second u56 (sacrifice last byte)
    // 4. 254: static str, ptr = first usize, len = second u56 (sacrifice last byte)
    // 5. 255: not used, for niche optimization (future)

    /// Valid UTF-8:
    /// 1. 0xxxxxxx        (0x00-0x7F)
    /// 2. 110xxxxx 10xxxxxx
    /// 3. 1110xxxx 10xxxxxx 10xxxxxx  
    /// 4. 11110xxx 10xxxxxx 10xxxxxx 10xxxxxx
    const LAST_BYTE_FIRST_INVALID_UTF8: u8 = 192; // 0b1100_0000

    const LAST_BYTE_HEAP: u8 = 253;
    const LAST_BYTE_STATIC: u8 = 254;
    const _LAST_BYTE_NONE: u8 = 255;

    const EMPTY: Self = const {
        let mut this = Self { bytes: [0; N] };
        *this.last_byte_mut() = Self::LAST_BYTE_FIRST_INVALID_UTF8;
        this
    };

    pub fn new(text: &str) -> Self {
        let len = text.len();

        debug_assert!((16..=61).contains(&N));
        debug_assert!(len < (1 << 56)); // len < 64PiB

        if len == 0 {
            return Self::EMPTY;
        }

        let mut this = Self { bytes: [0u8; N] };

        if len <= N {
            *this.last_byte_mut() = len as u8 | Self::LAST_BYTE_FIRST_INVALID_UTF8;
            unsafe { ptr::copy_nonoverlapping(text.as_ptr(), this.bytes.as_mut_ptr(), len) };
        } else {
            let s = text.as_bytes().to_vec().into_boxed_slice();

            *this.first_usize_mut() = Box::leak(s) as *mut [u8] as *mut u8 as _;
            #[cfg(target_endian = "little")]
            {
                *this.second_usize_mut() = len;
            }
            #[cfg(target_endian = "big")]
            {
                *this.second_usize_mut() = len << 8;
            }
            *this.last_byte_mut() = Self::LAST_BYTE_HEAP;
        }

        this
    }

    pub fn new_static(text: &'static str) -> Self {
        let len = text.len();

        debug_assert!((16..=61).contains(&N));
        debug_assert!(len < (1 << 56)); // len < 64PiB

        if len == 0 {
            return Self::EMPTY;
        }

        let mut this = Self { bytes: [0u8; N] };

        *this.first_usize_mut() = text.as_ptr() as usize;
        #[cfg(target_endian = "little")]
        {
            *this.second_usize_mut() = len;
        }
        #[cfg(target_endian = "big")]
        {
            *this.second_usize_mut() = len << 8;
        }
        *this.last_byte_mut() = Self::LAST_BYTE_STATIC;
        this
    }

    pub fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(self.as_slice()) }
    }

    pub fn as_slice(&self) -> &[u8] {
        let ptr: *const u8;
        let mut len: usize;

        if *self.last_byte() >= Self::LAST_BYTE_HEAP {
            // On heap.
            ptr = (*self.first_usize()) as *const u8;
            len = *self.second_usize();
            #[cfg(target_endian = "little")]
            {
                len &= 0x00FF_FFFF_FFFF_FFFF;
            }
            #[cfg(target_endian = "big")]
            {
                len >>= 8;
            }
        } else {
            // On stack.
            ptr = self as *const Self as *const u8;
            len = cmp::min(
                self.last_byte()
                    .wrapping_sub(Self::LAST_BYTE_FIRST_INVALID_UTF8) as usize,
                N,
            );
        };

        unsafe { slice::from_raw_parts(ptr, len) }
    }

    const fn first_usize(&self) -> &usize {
        let ptr = &self.bytes[0];
        unsafe { mem::transmute(ptr) }
    }

    const fn first_usize_mut(&mut self) -> &mut usize {
        let ptr = &mut self.bytes[0];
        unsafe { mem::transmute(ptr) }
    }

    const fn second_usize(&self) -> &usize {
        let ptr = &self.bytes[8];
        unsafe { mem::transmute(ptr) }
    }

    const fn second_usize_mut(&mut self) -> &mut usize {
        let ptr = &mut self.bytes[8];
        unsafe { mem::transmute(ptr) }
    }

    const fn last_byte(&self) -> &u8 {
        unsafe { self.bytes.last().unwrap_unchecked() }
    }

    const fn last_byte_mut(&mut self) -> &mut u8 {
        unsafe { self.bytes.last_mut().unwrap_unchecked() }
    }
}

impl<const N: usize> Drop for CompactString<N> {
    fn drop(&mut self) {
        if *self.last_byte() == Self::LAST_BYTE_HEAP {
            let ptr = *self.first_usize() as *mut u8;
            let mut len = *self.second_usize();

            #[cfg(target_endian = "little")]
            {
                len &= 0x00FF_FFFF_FFFF_FFFF;
            }
            #[cfg(target_endian = "big")]
            {
                len >>= 8;
            }

            unsafe {
                let slice_ptr = ptr::slice_from_raw_parts_mut(ptr, len);
                let _ = Box::from_raw(slice_ptr);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn gen_str(len: usize) -> String {
        "a".repeat(len)
    }

    #[test]
    fn test_empty_string() {
        let cs_new = CompactString::<32>::new("");
        assert_eq!(cs_new.as_str(), "");
        assert_eq!(cs_new.as_slice().len(), 0);
        assert_eq!(
            *cs_new.last_byte(),
            CompactString::<32>::LAST_BYTE_FIRST_INVALID_UTF8,
            "Empty string from new() should have tag 192"
        );

        let cs_const = CompactString::<32>::EMPTY;
        assert_eq!(cs_const.as_str(), "");
        assert_eq!(cs_const.as_slice().len(), 0);
        assert_eq!(
            *cs_const.last_byte(),
            CompactString::<32>::LAST_BYTE_FIRST_INVALID_UTF8,
            "EMPTY constant should have tag 192"
        );

        assert_eq!(cs_new.bytes, cs_const.bytes);
    }

    #[test]
    fn test_stack_storage_len_less_than_n() {
        const N: usize = 24;
        let s = "hello world"; // len = 11
        let cs = CompactString::<N>::new(s);

        assert_eq!(cs.as_str(), s);
        assert_eq!(cs.as_slice().len(), s.len());
        assert_eq!(*cs.last_byte(), 192 + s.len() as u8);
    }

    #[test]
    fn test_stack_storage_len_equals_n() {
        const N: usize = 32;
        let s = gen_str(N);
        let cs = CompactString::<N>::new(&s);

        assert_eq!(cs.as_str(), s.as_str());
        assert_eq!(cs.as_slice().len(), N);
        assert_eq!(*cs.last_byte(), b'a');
    }

    #[test]
    fn test_stack_storage_len_equals_n_with_multibyte_char() {
        const N: usize = 32;
        let s = gen_str(N - 3) + "€";
        assert_eq!(s.len(), N);

        let cs = CompactString::<N>::new(&s);

        assert_eq!(cs.as_str(), s.as_str());
        assert_eq!(cs.as_slice().len(), N);
        assert_eq!(*cs.last_byte(), 0xAC);
    }

    #[test]
    fn test_heap_storage() {
        const N: usize = 20;
        let s = gen_str(N + 1);
        let cs = CompactString::<N>::new(&s);

        assert_eq!(cs.as_str(), s.as_str());
        assert_eq!(cs.as_slice().len(), s.len());
        assert_eq!(*cs.last_byte(), CompactString::<N>::LAST_BYTE_HEAP);
    }

    #[test]
    fn test_static_storage() {
        const N: usize = 16;
        const STATIC_STR: &'static str = "this is a static string literal that is long";
        let cs = CompactString::<N>::new_static(STATIC_STR);

        assert_eq!(cs.as_str(), STATIC_STR);
        assert_eq!(cs.as_slice().len(), STATIC_STR.len());
        assert_eq!(*cs.last_byte(), CompactString::<N>::LAST_BYTE_STATIC);
        assert_eq!(*cs.first_usize(), STATIC_STR.as_ptr() as usize);
    }

    #[test]
    fn test_static_empty_string() {
        let cs = CompactString::<16>::new_static("");
        assert_eq!(cs.as_str(), "");
        assert_eq!(
            *cs.last_byte(),
            CompactString::<16>::LAST_BYTE_FIRST_INVALID_UTF8
        );
    }

    #[test]
    fn test_n_boundary_conditions_min() {
        const N: usize = 16;

        // len < N
        let s15 = gen_str(15);
        let cs15 = CompactString::<N>::new(&s15);
        assert_eq!(cs15.as_str(), s15.as_str());
        assert_eq!(*cs15.last_byte(), 192 + 15);

        // len == N
        let s16 = gen_str(16);
        let cs16 = CompactString::<N>::new(&s16);
        assert_eq!(cs16.as_str(), s16.as_str());
        assert_eq!(*cs16.last_byte(), b'a');

        // len > N
        let s17 = gen_str(17);
        let cs17 = CompactString::<N>::new(&s17);
        assert_eq!(cs17.as_str(), s17.as_str());
        assert_eq!(*cs17.last_byte(), CompactString::<N>::LAST_BYTE_HEAP);
    }

    #[test]
    fn test_n_boundary_conditions_max() {
        const N: usize = 61;

        // len < N
        let s60 = gen_str(60);
        let cs60 = CompactString::<N>::new(&s60);
        assert_eq!(cs60.as_str(), s60.as_str());
        assert_eq!(*cs60.last_byte(), 192 + 60);

        // len == N
        let s61 = gen_str(61);
        let cs61 = CompactString::<N>::new(&s61);
        assert_eq!(cs61.as_str(), s61.as_str());
        assert_eq!(*cs61.last_byte(), b'a');

        // len > N
        let s62 = gen_str(62);
        let cs62 = CompactString::<N>::new(&s62);
        assert_eq!(cs62.as_str(), s62.as_str());
        assert_eq!(*cs62.last_byte(), CompactString::<N>::LAST_BYTE_HEAP);
    }

    #[test]
    fn test_drop_heap_string_does_not_panic() {
        let handle = thread::spawn(|| {
            let s = gen_str(100);
            for _ in 0..1000 {
                let cs = CompactString::<32>::new(&s);
                assert_eq!(*cs.last_byte(), CompactString::<32>::LAST_BYTE_HEAP);
            }
        });
        handle.join().unwrap();
    }

    #[test]
    fn test_drop_static_string_does_not_panic() {
        const STATIC_STR: &'static str = "I must live forever";
        let handle = thread::spawn(move || {
            {
                let cs = CompactString::<16>::new_static(STATIC_STR);
                assert_eq!(*cs.last_byte(), CompactString::<16>::LAST_BYTE_STATIC);
            } // cs is dropped here
        });
        handle.join().unwrap();
        assert_eq!(STATIC_STR, "I must live forever");
    }
}
