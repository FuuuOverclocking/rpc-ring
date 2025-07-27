use std::{cmp, mem, ptr, slice};

#[repr(C, align(8))]
pub struct CompactString<const N: usize> {
    bytes: [u8; N],
}

impl<const N: usize> CompactString<N> {
    const EMPTY: Self = Self { bytes: [0; N] };

    pub fn new(text: &str) -> Self {
        debug_assert!(N >= 24);

        let len = text.len();

        if len == 0 {
            Self::EMPTY
        } else if len <= N {
            let mut bytes = [0u8; N];
            bytes[N - 1] = len as u8 | 0b11000000;
            unsafe { ptr::copy_nonoverlapping(text.as_ptr(), bytes.as_mut_ptr(), len) };

            Self { bytes }
        } else {
            let mut this = Self { bytes: [0u8; N] };
            let s = text.as_bytes().to_vec().into_boxed_slice();

            *this.ptr_mut() = Box::leak(s) as *mut [u8] as *mut u8 as _;
            *this.cap_mut() = len;
            *this.len_mut() = len;
            this
        }
    }

    pub fn as_str(&self) -> &str {
        unsafe { str::from_utf8_unchecked(self.as_slice()) }
    }

    pub fn as_slice(&self) -> &[u8] {
        let ptr: *const u8;
        let len: usize;

        if self.last_byte() >= 0b11011000 {
            // On heap.
            ptr = (*self.ptr()) as *const u8;
            len = *self.len();
        } else {
            // On stack.
            ptr = self as *const Self as *const u8;
            len = cmp::min(self.last_byte().wrapping_sub(0b11000000) as usize, N);
        };

        unsafe { slice::from_raw_parts(ptr, len) }
    }

    fn ptr(&self) -> &usize {
        let ptr = &self.bytes[0];
        unsafe { mem::transmute(ptr) }
    }

    fn ptr_mut(&mut self) -> &mut usize {
        let ptr = &mut self.bytes[0];
        unsafe { mem::transmute(ptr) }
    }

    fn cap(&self) -> &usize {
        let ptr = &self.bytes[8];
        unsafe { mem::transmute(ptr) }
    }

    fn cap_mut(&mut self) -> &mut usize {
        let ptr = &mut self.bytes[8];
        unsafe { mem::transmute(ptr) }
    }

    fn len(&self) -> &usize {
        let ptr = &self.bytes[16];
        unsafe { mem::transmute(ptr) }
    }

    fn len_mut(&mut self) -> &mut usize {
        let ptr = &mut self.bytes[16];
        unsafe { mem::transmute(ptr) }
    }

    fn last_byte(&self) -> u8 {
        unsafe { *self.bytes.get_unchecked(N - 1) }
    }
}
