use core::{
    ffi::{CStr, c_char},
    fmt::{self, Write},
};

use alloc::{ffi::CString, string::String, vec::Vec};
use csz::{CStrArray, CStrBox, CStrThin};

/// An internal buffer for [CStrTemp].
enum Buf<const N: usize> {
    Stack(CStrArray<N>),
    Heap(CString),
}

#[doc(hidden)]
/// A `CStrTemp` is a temporary C string stored on the stack or the heap.
pub struct CStrTemp<const N: usize = 2048> {
    buf: Buf<N>,
}

impl<const N: usize> CStrTemp<N> {
    fn new(s: &str) -> Self {
        let buf = CStrArray::from_bytes(s.as_bytes())
            .map(Buf::Stack)
            .unwrap_or_else(|_| Buf::Heap(CString::new(s).unwrap()));
        Self { buf }
    }

    /// Converts this C string to a byte slice containing the trailing 0 byte.
    pub fn as_bytes_with_null(&self) -> &[u8] {
        match &self.buf {
            Buf::Stack(s) => s.to_bytes_with_nul(),
            Buf::Heap(s) => s.as_bytes_with_nul(),
        }
    }

    /// Returns the inner pointer to this C string.
    pub fn as_ptr(&self) -> *const c_char {
        match &self.buf {
            Buf::Stack(s) => s.as_ptr(),
            Buf::Heap(s) => s.as_ptr(),
        }
    }

    /// Extracts a [CStr] slice containing the entire string.
    pub fn as_c_str(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.as_ptr()) }
    }

    /// Returns `true` if the string is stored on the stack.
    pub fn is_stack(&self) -> bool {
        matches!(self.buf, Buf::Stack(_))
    }

    /// Returns `true` if the string is stored on the heap.
    pub fn is_heap(&self) -> bool {
        matches!(self.buf, Buf::Heap(_))
    }
}

impl AsRef<CStrThin> for CStrTemp {
    fn as_ref(&self) -> &CStrThin {
        self.as_c_str().into()
    }
}

/// Shorthand for `s.as_ref().as_ptr()`.
#[doc(hidden)]
pub trait AsCStrPtr {
    fn as_ptr(&self) -> *const c_char;
}

impl<T: AsRef<CStrThin>> AsCStrPtr for T {
    fn as_ptr(&self) -> *const c_char {
        self.as_ref().as_ptr()
    }
}

/// Convert a type to a string accepted by the engine.
pub trait ToEngineStr {
    type Output: AsRef<CStrThin>;

    fn to_engine_str(&self) -> Self::Output;
}

impl ToEngineStr for &'_ str {
    type Output = CStrTemp;

    fn to_engine_str(&self) -> Self::Output {
        CStrTemp::new(self)
    }
}

impl ToEngineStr for String {
    type Output = CStrTemp;

    fn to_engine_str(&self) -> Self::Output {
        CStrTemp::new(self.as_str())
    }
}

impl<'a> ToEngineStr for &'a CStr {
    type Output = &'a CStr;

    fn to_engine_str(&self) -> Self::Output {
        self
    }
}

impl<'a> ToEngineStr for &'a CStrThin {
    type Output = &'a CStrThin;

    fn to_engine_str(&self) -> Self::Output {
        self
    }
}

impl<'a, const N: usize> ToEngineStr for &'a CStrArray<N> {
    type Output = &'a CStrThin;

    fn to_engine_str(&self) -> Self::Output {
        self.as_thin()
    }
}

impl<'a> ToEngineStr for &'a CStrBox {
    type Output = &'a CStrThin;

    fn to_engine_str(&self) -> Self::Output {
        self.as_c_str()
    }
}

impl<'a> ToEngineStr for &'a CString {
    type Output = &'a CStr;

    fn to_engine_str(&self) -> Self::Output {
        self.as_c_str()
    }
}

impl<'a> ToEngineStr for fmt::Arguments<'a> {
    type Output = CStrTemp;

    fn to_engine_str(&self) -> Self::Output {
        let mut temp = CStrArray::new();
        let buf = temp
            .cursor()
            .write_fmt(*self)
            .map(|_| Buf::Stack(temp))
            .unwrap_or_else(|_| {
                let mut s = String::with_capacity(temp.capacity() * 2);
                s.write_fmt(*self).unwrap();
                Buf::Heap(CString::new(s).unwrap())
            });
        CStrTemp { buf }
    }
}

pub trait ByteSliceExt {
    fn bytes_take_while(&self, pat: impl FnMut(u8) -> bool) -> (&Self, &Self);

    fn bytes_take_while_rev(&self, pat: impl FnMut(u8) -> bool) -> (&Self, &Self);

    fn bytes_trim_prefix(&self, pat: impl FnMut(u8) -> bool) -> &Self {
        self.bytes_take_while(pat).1
    }

    fn bytes_trim_suffix(&self, pat: impl FnMut(u8) -> bool) -> &Self {
        self.bytes_take_while_rev(pat).0
    }

    fn bytes_trim_ascii_start(&self) -> &Self {
        self.bytes_trim_prefix(|i| i.is_ascii_whitespace())
    }

    fn bytes_trim_ascii_end(&self) -> &Self {
        self.bytes_trim_suffix(|i| i.is_ascii_whitespace())
    }
}

impl ByteSliceExt for [u8] {
    fn bytes_take_while(&self, mut pat: impl FnMut(u8) -> bool) -> (&[u8], &[u8]) {
        let offset = self.iter().position(|&i| !pat(i)).unwrap_or(self.len());
        self.split_at(offset)
    }

    fn bytes_take_while_rev(&self, mut pat: impl FnMut(u8) -> bool) -> (&[u8], &[u8]) {
        let offset = self.iter().rev().position(|&i| !pat(i)).unwrap_or(0);
        self.split_at(self.len() - offset)
    }
}

#[derive(Copy, Clone)]
pub struct StringId(u32);

impl StringId {
    fn offset(&self) -> usize {
        self.0 as usize
    }
}

#[derive(Default)]
pub struct Strings {
    data: Vec<u8>,
}

impl Strings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn shrink_to_fit(&mut self) {
        self.data.shrink_to_fit();
    }

    fn next_id(&self) -> StringId {
        StringId(self.data.len().try_into().unwrap())
    }

    pub fn from_bytes_until_nul(&mut self, bytes: &[u8]) -> StringId {
        let id = self.next_id();
        for &i in bytes {
            if i == 0 {
                break;
            }
            self.data.push(i);
        }
        self.data.push(0);
        id
    }

    pub fn from_thin(&mut self, s: &CStrThin) -> StringId {
        let id = self.next_id();
        self.data.extend(s.bytes());
        self.data.push(0);
        id
    }

    pub fn from_c_str(&mut self, s: &CStr) -> StringId {
        let id = self.next_id();
        self.data.extend_from_slice(s.to_bytes_with_nul());
        id
    }

    pub fn try_get(&self, id: StringId) -> Option<&CStrThin> {
        let offset = id.offset();
        self.data.get(offset).map(|_| unsafe {
            let s = self.data.as_ptr().wrapping_add(offset);
            CStrThin::from_ptr(s as *const c_char)
        })
    }

    pub fn get(&self, id: StringId) -> &CStrThin {
        self.try_get(id).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use core::ffi::CStr;

    #[test]
    fn compat_str() {
        fn test<const N: usize>(src: &CStr, stack: bool) {
            let s = CStrTemp::<N>::new(src.to_str().unwrap());
            assert_eq!(s.is_stack(), stack);
            assert_eq!(s.as_bytes_with_null(), src.to_bytes_with_nul());
            assert_eq!(s.as_c_str(), src);
        }
        test::<8>(c"0123456", true);
        test::<7>(c"0123456", false);
    }

    #[test]
    fn bytes_ext() {
        use super::ByteSliceExt;

        let s = b" \t 123".bytes_take_while(|i| i.is_ascii_whitespace());
        assert_eq!(s, (&b" \t "[..], &b"123"[..]));
        let s = b"123abc".bytes_take_while_rev(|i| i.is_ascii_alphabetic());
        assert_eq!(s, (&b"123"[..], &b"abc"[..]));
        assert_eq!(
            b"123abc".bytes_trim_prefix(|i| i.is_ascii_digit()),
            &b"abc"[..]
        );
        assert_eq!(
            b"abc123".bytes_trim_suffix(|i| i.is_ascii_digit()),
            &b"abc"[..]
        );
    }
}
