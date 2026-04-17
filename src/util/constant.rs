//! Constants
//!
//! * [`Constant`]
//!
//! Pretty-printing
//!
//! * [`default_escape`]
//! * [`escape`]
//! * [`print_string_constant`]
//! * [`print()`]
//! * [`print_def`]

use crate::number::*;

use std::borrow::Cow;

use bumpalo::Bump;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Constant {
    Int(IntConstant),
    Real(RealConstant),
    Str(Box<str>),
}

pub fn default_escape(c: u8) -> Cow<'static, str> {
    match c {
        b'\\' => Cow::Borrowed(r"\\"),
        b'\n' => Cow::Borrowed(r"\n"),
        b'\r' => Cow::Borrowed(r"\r"),
        b'\t' => Cow::Borrowed(r"\t"),
        b'\x08' => Cow::Borrowed(r"\b"),
        b'"' => Cow::Borrowed(r#"\""#),
        32..=126 => Cow::Owned(format!("{}", c as char)),
        _ => Cow::Owned(format!(r"\x{c:0>2X}")),
    }
}

pub fn escape(f: impl Fn(u8) -> Cow<'static, str>, s: &str) -> String {
    let mut b = String::with_capacity(s.len());
    s.bytes().for_each(|c| b.push_str(&f(c)));
    b
}

pub fn print_string_constant<'a>(
    string_escape: impl Fn(u8) -> Cow<'static, str> + Copy + 'a,
    doc: &mut ocaml_format::Doc<'a>,
    s: &'a str,
) {
    doc.atom_fn(move |f| write!(f, r#""{}""#, escape(string_escape, s)));
}

pub fn print<'a>(
    support: &NumberSupport,
    string_escape: impl Fn(u8) -> Cow<'static, str> + Copy + 'a,
    doc: &mut ocaml_format::Doc<'a>,
    arena: &'a Bump,
    x: &'a Constant,
) {
    match x {
        Constant::Int(i) => print_int_constant(support, doc, arena, i),
        Constant::Real(r) => print_real_constant(support, doc, arena, r),
        Constant::Str(s) => print_string_constant(string_escape, doc, s),
    }
}

pub fn print_def<'a>(doc: &mut ocaml_format::Doc<'a>, arena: &'a Bump, x: &'a Constant) {
    print(&FULL_SUPPORT, default_escape, doc, arena, x);
}
