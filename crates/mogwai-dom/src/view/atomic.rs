use std::sync::atomic::{AtomicPtr, Ordering};

/// The same as `Atomic<Option<A>>` except faster and uses less memory.
///
/// This is because it represents `None` as a null pointer, which avoids boxing.
#[derive(Debug)]
pub(crate) struct AtomicOption<A> {
    ptr: AtomicPtr<A>,
}

impl<A> AtomicOption<A> {
    fn to_ptr(value: Option<A>) -> *mut A {
        match value {
            Some(value) => Box::into_raw(Box::new(value)),
            None => std::ptr::null_mut(),
        }
    }

    fn from_ptr(ptr: *mut A) -> Option<A> {
        if ptr.is_null() {
            None
        } else {
            // SAFETY: This is safe because we only do this for pointers created with `Box::into_raw`
            unsafe { Some(*Box::from_raw(ptr)) }
        }
    }

    #[inline]
    pub(crate) fn new(value: Option<A>) -> Self {
        Self {
            ptr: AtomicPtr::new(Self::to_ptr(value)),
        }
    }

    pub(crate) fn swap(&self, value: Option<A>) -> Option<A> {
        let new_ptr = Self::to_ptr(value);
        let old_ptr = self.ptr.swap(new_ptr, Ordering::AcqRel);
        Self::from_ptr(old_ptr)
    }

    #[inline]
    pub(crate) fn store(&self, value: Option<A>) {
        drop(self.swap(value));
    }

    #[inline]
    pub(crate) fn take(&self) -> Option<A> {
        self.swap(None)
    }
}

impl<A> Drop for AtomicOption<A> {
    fn drop(&mut self) {
        let ptr = self.ptr.load(Ordering::Acquire);

        if !ptr.is_null() {
            // SAFETY: This is safe because we only do this for pointers created with `Box::into_raw`
            unsafe {
                drop(Box::from_raw(ptr));
            }
        }
    }
}
