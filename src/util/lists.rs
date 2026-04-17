//! # Combinators on slice type
//!
//! * [`equal`]

pub fn equal<T, U>(pr: impl Fn(&T, &U) -> bool, l1: &[T], l2: &[U]) -> bool {
    l1.len() == l2.len() && l1.iter().zip(l2.iter()).all(|(x, y)| pr(x, y))
}
