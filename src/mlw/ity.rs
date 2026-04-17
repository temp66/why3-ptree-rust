//! # Types in WhyML programs
//!
//! Individual types are first-order types without effects.
//!
//! Computation types are higher-order types with effects.
//!
//! ## Exception symbols
//!
//! A mask is a generalized ghost information allowing to handle
//!    tuples where some components can be ghost and others are not.
//!
//!    They are used for expressions, including results of programs, and
//!    for exceptions
//!
//! * [`Mask`]

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Mask {
    /// fully non-ghost
    Visible,
    /// decomposed ghost status for tuples
    Tuple(Box<[Mask]>),
    /// fully ghost
    Ghost,
}
