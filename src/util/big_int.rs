//! # Wrapper for big nums, implemented with `malachite`
//!
//! ## Division and modulo operators
//!
//! ### Euclidean division
//!
//! By convention, the modulo is always non-negative.
//! This implies that division rounds down when divisor is positive, and
//! rounds up when divisor is negative.
//!
//! * [`euclidean_mod`]
//!
//! [`euclidean_mod`]: trait.RemN.html#impl-RemN<%26Integer>-for-Integer

use malachite::{Integer, base::num::basic::traits::Zero};

pub trait RemN<Rhs = Self> {
    type Output;

    fn euclidean_mod(self, rhs: Rhs) -> Self::Output;
}

impl RemN<&Self> for Integer {
    type Output = Self;

    fn euclidean_mod(mut self, other: &Self) -> Self {
        self %= other;
        if self < Integer::ZERO {
            self += other;
        }
        self
    }
}
