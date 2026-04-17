//! # Logic Declarations
//!
//! ## Inductive predicate declaration
//!
//! * [`IndSign`]
//!
//! ## Proposition declaration
//!
//! * [`PropKind`]

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum IndSign {
    Ind,
    Coind,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PropKind {
    /// prove, use as a premise
    Lemma,
    /// do not prove, use as a premise
    Axiom,
    /// prove, do not use as a premise
    Goal,
}
