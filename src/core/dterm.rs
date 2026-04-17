//! Patterns, terms, and formulas
//!
//! * [`Dbinop`]
//! * [`Dquant`]

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Dbinop {
    And,
    AndAsym,
    Or,
    OrAsym,
    Implies,
    Iff,
    By,
    So,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Dquant {
    Forall,
    Exists,
    Lambda,
}
