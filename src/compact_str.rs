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
                len &= 0x00_FF_FF_FF__FF_FF_FF_FF;
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
                len &= 0x00FF_FF_FF_FF_FF_FF_FF;
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
