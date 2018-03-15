#![doc(html_root_url = "https://docs.rs/try-lock/0.1.0")]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![deny(warnings)]

//! A light-weight lock guarded by an atomic boolean.
//!
//! Most efficient when contention is low, acquiring the lock is a single
//! atomic swap, and releasing it just 1 more atomic swap.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use try_lock::TryLock;
//!
//! // a thing we want to share
//! struct Widget {
//!     name: String,
//! }
//!
//! // lock it up!
//! let widget1 = Arc::new(TryLock::new(Widget {
//!     name: "Spanner".into(),
//! }));
//!
//! let widget2 = widget1.clone();
//!
//!
//! // mutate the widget
//! let mut locked = widget1.try_lock().expect("example isn't locked yet");
//! locked.name.push_str(" Bundle");
//!
//! // hands off, buddy
//! let not_locked = widget2.try_lock();
//! assert!(not_locked.is_none(), "widget1 has the lock");
//!
//! // ok, you can have it
//! drop(locked);
//!
//! let locked2 = widget2.try_lock().expect("widget1 lock is released");
//!
//! assert_eq!(locked2.name, "Spanner Bundle");
//! ```

use std::cell::UnsafeCell;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};

/// A light-weight lock guarded by an atomic boolean.
///
/// Most efficient when contention is low, acquiring the lock is a single
/// atomic swap, and releasing it just 1 more atomic swap.
///
/// It is only possible to try to acquire the lock, it is not possible to
/// wait for the lock to become ready, like with a `Mutex`.
pub struct TryLock<T> {
    is_locked: AtomicBool,
    value: UnsafeCell<T>,
}

impl<T> TryLock<T> {
    /// Create a `TryLock` around the value.
    pub fn new(val: T) -> TryLock<T> {
        TryLock {
            is_locked: AtomicBool::new(false),
            value: UnsafeCell::new(val),
        }
    }

    /// Try to acquire the lock of this value.
    ///
    /// If the lock is already acquired by someone else, this returns
    /// `None`. You can try to acquire again whenever you want, perhaps
    /// by spinning a few times, or by using some other means of
    /// notification.
    pub fn try_lock(&self) -> Option<Locked<T>> {
        if !self.is_locked.swap(true, Ordering::SeqCst) {
            Some(Locked { lock: self })
        } else {
            None
        }
    }
}

unsafe impl<T: Send> Send for TryLock<T> {}
unsafe impl<T: Send> Sync for TryLock<T> {}

impl<T: fmt::Debug> fmt::Debug for TryLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {

        // Used if the TryLock cannot acquire the lock.
        struct LockedPlaceholder;

        impl fmt::Debug for LockedPlaceholder {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("<locked>")
            }
        }

        let mut builder = f.debug_struct("TryLock");
        if let Some(locked) = self.try_lock() {
            builder.field("value", &*locked);
        } else {
            builder.field("value", &LockedPlaceholder);
        }
        builder.finish()
    }
}

/// A locked value acquired from a `TryLock`.
///
/// The type represents an exclusive view at the underlying value. The lock is
/// released when this type is dropped.
///
/// This type derefs to the underlying value.
pub struct Locked<'a, T: 'a> {
    lock: &'a TryLock<T>,
}

impl<'a, T> Deref for Locked<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.value.get() }
    }
}

impl<'a, T> DerefMut for Locked<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<'a, T> Drop for Locked<'a, T> {
    fn drop(&mut self) {
        self.lock.is_locked.store(false, Ordering::SeqCst);
    }
}

impl<'a, T: fmt::Debug> fmt::Debug for Locked<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::TryLock;

    #[test]
    fn fmt_debug() {
        let lock = TryLock::new(5);
        assert_eq!(format!("{:?}", lock), "TryLock { value: 5 }");

        let locked = lock.try_lock().unwrap();
        assert_eq!(format!("{:?}", locked), "5");

        assert_eq!(format!("{:?}", lock), "TryLock { value: <locked> }");
    }
}
