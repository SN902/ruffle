use std::borrow::Cow;
use std::ops::Deref;

use gc_arena::{Collect, Gc, Mutation};
use ruffle_wstr::{wstr_impl_traits, WStr, WString};

use crate::string::{AvmAtom, AvmStringRepr};

#[derive(Clone, Copy, Collect)]
#[collect(no_drop)]
enum Source<'gc> {
    Owned(Gc<'gc, AvmStringRepr<'gc>>),
    Static(&'static WStr),
}

#[derive(Clone, Copy, Collect)]
#[collect(no_drop)]
pub struct AvmString<'gc> {
    source: Source<'gc>,
}

impl<'gc> AvmString<'gc> {
    /// Turns a string to a fully owned (non-dependent) managed string.
    pub(super) fn to_fully_owned(self, gc_context: &Mutation<'gc>) -> Gc<'gc, AvmStringRepr<'gc>> {
        match self.source {
            Source::Owned(s) => {
                if s.is_dependent() {
                    let repr = AvmStringRepr::from_raw(WString::from(self.as_wstr()), false);
                    Gc::new(gc_context, repr)
                } else {
                    s
                }
            }
            Source::Static(s) => {
                let repr = AvmStringRepr::from_raw(s.into(), false);
                Gc::new(gc_context, repr)
            }
        }
    }

    pub fn new_utf8<'s, S: Into<Cow<'s, str>>>(gc_context: &Mutation<'gc>, string: S) -> Self {
        let buf = match string.into() {
            Cow::Owned(utf8) => WString::from_utf8_owned(utf8),
            Cow::Borrowed(utf8) => WString::from_utf8(utf8),
        };
        let repr = AvmStringRepr::from_raw(buf, false);
        Self {
            source: Source::Owned(Gc::new(gc_context, repr)),
        }
    }

    pub fn new_utf8_bytes(gc_context: &Mutation<'gc>, bytes: &[u8]) -> Self {
        let buf = WString::from_utf8_bytes(bytes.to_vec());
        Self::new(gc_context, buf)
    }

    pub fn new<S: Into<WString>>(gc_context: &Mutation<'gc>, string: S) -> Self {
        let repr = AvmStringRepr::from_raw(string.into(), false);
        Self {
            source: Source::Owned(Gc::new(gc_context, repr)),
        }
    }

    pub fn new_dependent(
        gc_context: &Mutation<'gc>,
        string: AvmString<'gc>,
        start: usize,
        end: usize,
    ) -> Self {
        // TODO?: if string is static, just make a new static AvmString
        let repr = AvmStringRepr::new_dependent(string, start, end);
        Self {
            source: Source::Owned(Gc::new(gc_context, repr)),
        }
    }

    pub fn owner(&self) -> Option<AvmString<'gc>> {
        match &self.source {
            Source::Owned(s) => s.owner(),
            Source::Static(_) => None,
        }
    }

    pub fn as_wstr(&self) -> &WStr {
        match &self.source {
            Source::Owned(s) => s,
            Source::Static(s) => s,
        }
    }

    pub fn as_interned(&self) -> Option<AvmAtom<'gc>> {
        match self.source {
            Source::Owned(s) if s.is_interned() => Some(AvmAtom(s)),
            _ => None,
        }
    }

    pub fn concat(
        gc_context: &Mutation<'gc>,
        left: AvmString<'gc>,
        right: AvmString<'gc>,
    ) -> AvmString<'gc> {
        if left.is_empty() {
            right
        } else if right.is_empty() {
            left
        } else {
            // note: we could also in-place append a byte string to a wide string
            // But it was skipped for now.

            if left.is_wide() == right.is_wide() {
                let left_origin_s = left.owner().unwrap_or(left);
                if let (Source::Owned(left), Source::Owned(left_origin)) = (left.source, left_origin_s.source) {
                    let char_size = if left.is_wide() { 2 } else { 1 };
                    /*
                        assumptions:
                        - left.len <= left.chars_used <= left.capacity
                        - left_ptr is inside left_origin_ptr .. left_origin_ptr + left.chars_used

                        note: it's possible that left == left_origin.
                    */
                    unsafe {
                        let left_origin_ptr = left_origin.raw_ptr() as *const u8;
                        let left_ptr = left_origin.raw_ptr() as *const u8;

                        /*
                        Assume a="abc", b=a+"d", c=a.substr(1), we're running d=c+"e"

                        a          ->  abc
                        b          ->  abcd
                        c          ->   bc        v left_capacity_end
                        a's memory ->  abcd_______
                                          ^ first_requested
                                           ^ first_available

                        We can only append in-place if first_requested and first_available match
                        And we have enough spare capacity.
                        */

                        let first_available = left_origin_ptr.add(char_size * left_origin.chars_used() as usize);
                        let first_requested = left_ptr.add(char_size * left.len() as usize);

                        let mut chars_available = 0;
                        if first_available == first_requested {
                            let left_capacity_end = left_origin_ptr.add(char_size * left_origin.capacity() as usize);
                            chars_available = ((left_capacity_end as usize) - (first_available as usize)) / char_size;
                        }
                        if chars_available >= right.len() {
                            let first_available = first_available as *mut u8;
                            let right_ptr = right.as_wstr() as *const WStr as *const () as *const u8;
                            std::ptr::copy_nonoverlapping(right_ptr, first_available, char_size * right.len());

                            // TODO: usize/u32 safety? range safety?
                            left_origin.set_chars_used(left_origin.chars_used() + right.len() as u32);

                            let repr = AvmStringRepr::new_dependent_raw(left_origin_s, left_ptr, (left.len()+right.len()) as u32, left.is_wide());
                            return Self {
                                source: Source::Owned(Gc::new(gc_context, repr)),
                            };
                        }
                    }
                }
            }

            // When doing a non-in-place append,
            // Overallocate a bit so that further appends can be in-place.
            // (Note that this means that all first-time appends will happen here and
            // overallocate, even if done only once)
            // This growth logic should be equivalent to AVM's, except I capped the growth at 1MB instead of 4MB.
            let new_size = left.len() + right.len();
            let new_capacity = if new_size < 32 {
                32
            } else if new_size > 1024*1024 {
                new_size + 1024*1024
            } else {
                new_size * 2
            };

            let mut out = WString::with_capacity(new_capacity, left.is_wide() || right.is_wide());
            out.push_str(&left);
            out.push_str(&right);
            Self::new(gc_context, out)
        }
    }

    #[inline]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        std::ptr::eq(this.as_wstr(), other.as_wstr())
    }
}

impl<'gc> From<AvmAtom<'gc>> for AvmString<'gc> {
    #[inline]
    fn from(atom: AvmAtom<'gc>) -> Self {
        Self {
            source: Source::Owned(atom.0),
        }
    }
}

impl<'gc> From<Gc<'gc, AvmStringRepr<'gc>>> for AvmString<'gc> {
    #[inline]
    fn from(repr: Gc<'gc, AvmStringRepr<'gc>>) -> Self {
        Self {
            source: Source::Owned(repr),
        }
    }
}

impl Default for AvmString<'_> {
    fn default() -> Self {
        Self {
            source: Source::Static(WStr::empty()),
        }
    }
}

impl<'gc> From<&'static str> for AvmString<'gc> {
    #[inline]
    fn from(str: &'static str) -> Self {
        // TODO(moulins): actually check that `str` is valid ASCII.
        Self {
            source: Source::Static(WStr::from_units(str.as_bytes())),
        }
    }
}

impl<'gc> From<&'static WStr> for AvmString<'gc> {
    #[inline]
    fn from(str: &'static WStr) -> Self {
        Self {
            source: Source::Static(str),
        }
    }
}

impl<'gc> Deref for AvmString<'gc> {
    type Target = WStr;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_wstr()
    }
}

// Manual equality implementation with fast paths for owned strings.
impl<'gc> PartialEq for AvmString<'gc> {
    fn eq(&self, other: &Self) -> bool {
        if let (Source::Owned(left), Source::Owned(right)) = (self.source, other.source) {
            // Fast accept for identical strings.
            if Gc::ptr_eq(left, right) {
                return true;
            // Fast reject for distinct interned strings.
            } else if left.is_interned() && right.is_interned() {
                return false;
            }
        }

        // Fallback case.
        self.as_wstr() == other.as_wstr()
    }
}

impl<'gc> PartialEq<AvmString<'gc>> for AvmAtom<'gc> {
    fn eq(&self, other: &AvmString<'gc>) -> bool {
        if let Some(atom) = other.as_interned() {
            *self == atom
        } else {
            self.as_wstr() == other.as_wstr()
        }
    }
}

impl<'gc> PartialEq<AvmAtom<'gc>> for AvmString<'gc> {
    #[inline(always)]
    fn eq(&self, other: &AvmAtom<'gc>) -> bool {
        PartialEq::eq(other, self)
    }
}

impl<'gc> Eq for AvmString<'gc> {}

wstr_impl_traits!(impl['gc] manual_eq for AvmString<'gc>);
