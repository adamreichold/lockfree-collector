//! A lock-free blocked stealing collector
//!
//! This is basically a lock-free stack but tailored to be used as a collector,
//! i.e. the `collect` method steals all values using a single atomic operation
//! and it stores blocks of `B` values to amortize the cost of heap allocations.
//!
//! When choosing a block size `B`, consider that each block currently contains
//! two additional pointer-sized fields.
//!
//! ```
//! use std::thread;
//!
//! use lockfree_collector::Collector;
//!
//! let collector = Collector::<String, 30>::new();
//!
//! thread::scope(|scope| {
//!     for _ in 0..30 {
//!         scope.spawn(|| {
//!             for num in 0..10 {
//!                 collector.push(num.to_string());
//!             }
//!         });
//!     }
//! });
//!
//! let mut cnt = 0;
//!
//! collector.collect(|txt| cnt += txt.parse::<i32>().is_ok() as usize);
//!
//! assert_eq!(cnt, 30 * 10);
//! ```
#![deny(missing_docs, clippy::undocumented_unsafe_blocks)]
#![cfg_attr(not(test), no_std)]

extern crate alloc;

use core::mem::MaybeUninit;
use core::ptr::null_mut;

#[cfg(target_has_atomic = "ptr")]
use core::sync::atomic::{AtomicPtr, Ordering};
#[cfg(not(target_has_atomic = "ptr"))]
use portable_atomic::{AtomicPtr, Ordering};

use alloc::boxed::Box;

/// A lock-free blocked stealing collector
///
/// Dropping the collector will leak any uncollected values.
pub struct Collector<T, const B: usize>(AtomicPtr<Block<T, B>>);

#[repr(C, align(64))]
struct Block<T, const B: usize> {
    next: *mut Self,
    cnt: usize,
    vals: [MaybeUninit<T>; B],
}

impl<T, const B: usize> Collector<T, B> {
    /// Create an empty collector without allocating any blocks
    pub const fn new() -> Self {
        assert!(B != 0, "Block size must not be zero");

        Self(AtomicPtr::new(null_mut()))
    }
}

impl<T, const B: usize> Default for Collector<T, B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const B: usize> Collector<T, B>
where
    T: Send,
{
    /// Push a value into the collector
    pub fn push(&self, val: T) {
        let old_top = self.0.swap(null_mut(), Ordering::AcqRel);

        let mut curr = old_top;

        while !curr.is_null() {
            // SAFETY: We have ownership of the whole chain starting at `old_top`.
            let block = unsafe { &mut *curr };

            if block.cnt < B {
                block.vals[block.cnt].write(val);

                block.cnt += 1;

                self.update(old_top);
                return;
            }

            curr = block.next;
        }

        // There is no existing chain or it has no unused capacity remaining,
        // hence we allocate a new block and prepend it locally before publishing.

        // SAFETY: `MaybeUninit` itself needs no initialization.
        let mut vals: [MaybeUninit<T>; B] = unsafe { MaybeUninit::uninit().assume_init() };

        vals[0].write(val);

        let block = Block {
            next: old_top,
            cnt: 1,
            vals,
        };

        let top = Box::into_raw(Box::new(block));

        self.update(top);
    }

    /// Collect the values into an iterator
    ///
    /// Dropping the iterator will drop the remaining collected values.
    pub fn collect<F>(&self, mut f: F)
    where
        F: FnMut(T),
    {
        let old_top = self.0.swap(null_mut(), Ordering::AcqRel);

        if old_top.is_null() {
            return;
        }

        let mut curr = old_top;

        while !curr.is_null() {
            // SAFETY: We have ownership of the whole chain starting at `old_top`.
            let block = unsafe { &mut *curr };

            for val in &block.vals[..block.cnt] {
                // SAFETY: All values up to `cnt` have been initialized.
                f(unsafe { val.assume_init_read() });
            }

            block.cnt = 0;

            curr = block.next;
        }

        self.update(old_top);
    }

    fn update(&self, new_top: *mut Block<T, B>) {
        // SAFETY: We just allocated `new_top` and have not yet published it
        // or we have obtained ownership by atomically swapping it out of `self.0`.
        let mut last_next = unsafe { &mut (*new_top).next };

        while !last_next.is_null() {
            // SAFETY: We have ownership of the whole chain starting at `new_top`.
            last_next = unsafe { &mut (**last_next).next };
        }

        let mut old_top = self.0.load(Ordering::Relaxed);

        loop {
            *last_next = old_top;

            match self.0.compare_exchange_weak(
                old_top,
                new_top,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(top) => old_top = top,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::thread::scope;

    #[test]
    fn it_works_single_thread() {
        let collector = Collector::<String, 30>::new();

        for num in 0..100 {
            collector.push(num.to_string());
        }

        let mut sum = 0;

        collector.collect(|txt| sum += txt.parse::<i32>().unwrap());

        assert_eq!(sum, 99 * 100 / 2);
    }

    #[test]
    fn it_works_multiple_threads() {
        let collector = Collector::<String, 30>::new();

        scope(|scope| {
            for _ in 0..30 {
                scope.spawn(|| {
                    for num in 0..10 {
                        collector.push(num.to_string());
                    }
                });
            }
        });

        let mut sum = 0;

        collector.collect(|txt| sum += txt.parse::<i32>().unwrap());

        assert_eq!(sum, 30 * 9 * 10 / 2);
    }

    #[test]
    fn collect_incrementally() {
        let collector = Collector::<String, 30>::new();

        let mut sum = 0;

        scope(|scope| {
            for _ in 0..30 {
                scope.spawn(|| {
                    for num in 0..100 {
                        collector.push(num.to_string());
                    }
                });
            }

            collector.collect(|txt| sum += txt.parse::<i32>().unwrap());
        });

        collector.collect(|txt| sum += txt.parse::<i32>().unwrap());

        assert_eq!(sum, 30 * 99 * 100 / 2);
    }
}
