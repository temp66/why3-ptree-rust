//! # WhyML program expressions
//!
//! ## Routine symbols
//!
//! * [`RsKind`]
//!
//! ## Program expressions
//!
//! * [`AssertionKind`]
//! * [`ForDirection`]

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum RsKind {
    /// non-pure symbol
    None,
    /// local let-function
    Local,
    /// top-level let-function
    Func,
    /// top-level let-predicate
    Pred,
    /// top-level or local let-lemma
    Lemma,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum AssertionKind {
    Assert,
    Assume,
    Check,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ForDirection {
    To,
    DownTo,
}
