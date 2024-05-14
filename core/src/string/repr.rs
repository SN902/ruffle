use std::cell::Cell;
use std::ops::Deref;

use gc_arena::Collect;
use ruffle_wstr::{ptr as wptr, wstr_impl_traits, WStr, WString};

use crate::string::avm_string::AvmString;

/// Internal representation of `AvmAtom`s and (owned) `AvmString`.
///
/// Using this type directly is dangerous, as it can be used to violate
/// the interning invariants.
#[derive(Collect)]
#[collect(unsafe_drop)]
pub struct AvmStringRepr<'gc> {
    #[collect(require_static)]
    ptr: *mut (),

    // Length and is_wide bit.
    #[collect(require_static)]
    meta: wptr::WStrMetadata,

    // We abuse WStrMetadata to store capacity and is_interned bit.
    // If a string is Dependent, the capacity should always be 0.
    #[collect(require_static)]
    capacity: Cell<wptr::WStrMetadata>,

    // If a string is Dependent, this should always be 0.
    // If a string is Owned, this indicates used chars, including dependents.
    // Example: assume a string a="abc" has 10 bytes of capacity (chars_used=3).
    // Then, with a+"d", we produce a dependent string and owner's chars_used becomes 4.
    // len <= chars_used <= capacity.
    #[collect(require_static)]
    chars_used: Cell<u32>,

    // If Some, the string is dependent. The owner is assumed to be non-dynamic.
    owner: Option<AvmString<'gc>>,
}

impl<'gc> AvmStringRepr<'gc> {
    pub fn from_raw(s: WString, interned: bool) -> Self {
        let (ptr, meta, cap) = s.into_raw_parts();
        let capacity = Cell::new(wptr::WStrMetadata::new32(cap, interned));
        Self {
            ptr,
            meta,
            capacity,
            chars_used: Cell::new(meta.len32()),
            owner: None,
        }
    }

    pub fn new_dependent(s: AvmString<'gc>, start: usize, end: usize) -> Self {
        let wstr = &s[start..end];
        let wstr_ptr = wstr as *const WStr;

        let meta = unsafe { wptr::WStrMetadata::of(wstr_ptr) };
        // Dependent strings are never interned
        let capacity = Cell::new(wptr::WStrMetadata::new32(0, false));
        let ptr = wstr_ptr as *mut WStr as *mut ();

        let owner = if let Some(owner) = s.owner() {
            owner
        } else {
            s
        };

        Self {
            owner: Some(owner),
            ptr,
            meta,
            chars_used: Cell::new(0),
            capacity,
        }
    }

    pub unsafe fn new_dependent_raw(owner: AvmString<'gc>, ptr: *const u8, length: u32, is_wide: bool) -> Self {
        let meta = wptr::WStrMetadata::new32(length, is_wide);
        // Dependent strings are never interned
        let capacity = Cell::new(wptr::WStrMetadata::new32(0, false));
        let ptr = ptr as *mut ();

        Self {
            owner: Some(owner),
            ptr,
            meta,
            chars_used: Cell::new(0),
            capacity,
        }
    }

    #[inline]
    pub fn is_dependent(&self) -> bool {
        self.owner.is_some()
    }

    #[inline]
    pub fn owner(&self) -> Option<AvmString<'gc>> {
        self.owner
    }

    #[inline]
    pub fn as_wstr(&self) -> &WStr {
        // SAFETY: we own a `WString`.
        unsafe { &*wptr::from_raw_parts(self.ptr, self.meta) }
    }

    pub fn is_interned(&self) -> bool {
        self.capacity.get().is_wide()
    }

    pub fn mark_interned(&self) {
        if self.is_dependent() {
            panic!("bug: we interned a dependent string");
        }
        let cap = self.capacity.get();
        let new_cap = wptr::WStrMetadata::new32(cap.len32(), true);
        self.capacity.set(new_cap);
    }

    pub fn raw_ptr(&self) -> *mut () {
        self.ptr
    }

    pub fn capacity(&self) -> u32 {
        self.capacity.get().len32()
    }

    pub fn chars_used(&self) -> u32 {
        self.chars_used.get()
    }

    pub fn set_chars_used(&self, value: u32) {
        self.chars_used.set(value);
    }
}

impl<'gc> Drop for AvmStringRepr<'gc> {
    fn drop(&mut self) {
        if self.owner.is_none() {
            // SAFETY: we drop the `WString` we logically own.
            unsafe {
                let cap = self.capacity.get().len32();
                let _ = WString::from_raw_parts(self.ptr, self.meta, cap);
            }
        }
    }
}

impl<'gc> Deref for AvmStringRepr<'gc> {
    type Target = WStr;
    #[inline]
    fn deref(&self) -> &WStr {
        self.as_wstr()
    }
}

impl<'gc> Default for AvmStringRepr<'gc> {
    #[inline]
    fn default() -> Self {
        Self::from_raw(WString::new(), false)
    }
}

wstr_impl_traits!(impl['gc] for AvmStringRepr<'gc>);
