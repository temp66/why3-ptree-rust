//! # Identifiers
//!
//! ## Attributes
//!
//! * [`Attribute`]
//! * [`create_attribute`]
//!
//! ## Naming convention
//!
//! * [`Notation`]
//! * [`op_infix`]
//! * [`OP_TIGHT`]
//! * [`op_prefix`]
//! * [`op_get`]
//! * [`op_set`]
//! * [`op_update`]
//! * [`op_cut`]
//! * [`op_lcut`]
//! * [`op_rcut`]
//! * [`OP_EQU`]
//! * [`OP_NEQ`]
//! * [`sn_decode`]
//!
//! ## General purpose attributes
//!
//! * [`FUNLIT`]

use std::sync::LazyLock;

use internment::ArcIntern;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Attribute(pub ArcIntern<str>);

pub fn create_attribute(s: ArcIntern<str>) -> Attribute {
    Attribute(s)
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Notation {
    // plus
    Word(Box<str>),
    // +
    Infix(Box<str>),
    // !
    Tight(Box<str>),
    // -_
    Prefix(Box<str>),
    // []
    Get(Box<str>),
    // []<-
    Set(Box<str>),
    // [<-]
    Update(Box<str>),
    // [..]
    Cut(Box<str>),
    // [.._]
    Lcut(Box<str>),
    // [_..]
    Rcut(Box<str>),
}

// current encoding

pub fn op_infix(s: &str) -> String {
    format!("infix {s}")
}

pub fn op_prefix(s: &str) -> String {
    format!("prefix {s}")
}

pub fn op_get(s: &str) -> String {
    format!("mixfix []{s}")
}

pub fn op_set(s: &str) -> String {
    format!("mixfix []<-{s}")
}

pub fn op_update(s: &str) -> String {
    format!("mixfix [<-]{s}")
}

pub fn op_cut(s: &str) -> String {
    format!("mixfix [..]{s}")
}

pub fn op_lcut(s: &str) -> String {
    format!("mixfix [.._]{s}")
}

pub fn op_rcut(s: &str) -> String {
    format!("mixfix [_..]{s}")
}

pub static OP_EQU: LazyLock<String> = LazyLock::new(|| op_infix("="));

pub static OP_NEQ: LazyLock<String> = LazyLock::new(|| op_infix("<>"));

pub const OP_TIGHT: fn(&str) -> String = op_prefix;

fn print_sn<'a>(doc: &mut ocaml_format::Doc<'a>, w: &'a Notation) {
    fn lspace(p: &str) -> &str {
        if p.as_bytes()[0] == b'*' { " " } else { "" }
    }

    fn rspace(p: &str) -> &str {
        if p.bytes().last().unwrap() == b'*' {
            " "
        } else {
            ""
        }
    }

    // infix/prefix never empty, mixfix cannot have stars
    match w {
        Notation::Infix(p) => doc.atom_fn(move |f| write!(f, "({}{p}{})", lspace(p), rspace(p))),
        Notation::Tight(p) => doc.atom_fn(move |f| write!(f, "({p}{})", rspace(p))),
        Notation::Prefix(p) => doc.atom_fn(move |f| write!(f, "({}{p}_)", lspace(p))),
        Notation::Get(p) => doc.atom_fn(move |f| write!(f, "([]{p})")),
        Notation::Set(p) => doc.atom_fn(move |f| write!(f, "([]{p}<-)")),
        Notation::Update(p) => doc.atom_fn(move |f| write!(f, "([<-]{p})")),
        Notation::Cut(p) => doc.atom_fn(move |f| write!(f, "([..]{p})")),
        Notation::Lcut(p) => doc.atom_fn(move |f| write!(f, "([.._]{p})")),
        Notation::Rcut(p) => doc.atom_fn(move |f| write!(f, "([_..]{p})")),
        Notation::Word(p) => doc.atom(p),
    };
}

// The function below recognizes the following strings as notations:
//       "infix " (opchar+ [']* as p) (['_] [^'_] .* as q)
//       "prefix " (opchar+ [']* as p) (['_] [^'_] .* as q)
//       "mixfix " .* "]" opchar* ([']* as p) (['_] [^'_] .* as q)
//    It will fail if you add a mixfix that contains a non-opchar after
//    the closing square bracket, or a mixfix that does not use brackets.
//    Be careful when working with this code, it may eat your brain.

/// decode the string as a symbol name
pub fn sn_decode(s: &str) -> Notation {
    let len = s.len();
    if len <= 6 || s.as_bytes()[5] != b' ' && s.as_bytes()[6] != b' ' {
        Notation::Word(s.into())
    } else {
        let k = match &s[..6] {
            "infix " => 6,
            "prefix" => 7,
            "mixfix" => s[7..].find(']').map_or(0, |i| 7 + i + 1),
            _ => 0,
        };
        if k == 0 {
            Notation::Word(s.into())
        } else {
            let skip_opchar = |mut i| {
                while i < len
                    && matches!(
                        s.as_bytes()[i],
                        b'@' | b'!'
                            | b'^'
                            | b'$'
                            | b'='
                            | b'%'
                            | b'>'
                            | b'#'
                            | b'.'
                            | b'<'
                            | b'-'
                            | b'&'
                            | b'/'
                            | b'+'
                            | b'?'
                            | b':'
                            | b'*'
                            | b'~'
                            | b'|'
                            | b'\\',
                    )
                {
                    i += 1;
                }
                i
            };
            let l = skip_opchar(k);
            let skip_quote = |mut i| {
                while i < len {
                    if s.as_bytes()[i] == b'\'' {
                        i += 1;
                        continue;
                    }
                    return if i == l || s.as_bytes()[i] == b'_' {
                        i
                    } else {
                        i - 1
                    };
                }
                i
            };
            let m = skip_quote(l);
            let prefix = |o: &str| {
                if o.as_bytes()[0] != b'!' && o.as_bytes()[0] != b'?' {
                    Notation::Prefix(o.into())
                } else {
                    for i in 1..(l - 7) {
                        if !matches!(
                            o.as_bytes()[i],
                            b'!' | b'$' | b'&' | b'?' | b'@' | b'^' | b'.' | b':' | b'|' | b'#',
                        ) {
                            return Notation::Prefix(o.into());
                        }
                    }
                    Notation::Tight(o.into())
                }
            };
            if l == k && k < 8 {
                // null infix/prefix
                Notation::Word(s.into())
            } else {
                let w = {
                    if k == 6 {
                        Notation::Infix(s[6..m].into())
                    } else if k == 7 {
                        prefix(&s[7..m])
                    } else {
                        let p = if l < m { &s[l..m] } else { "" };
                        match &s[8..l] {
                            "]" => Notation::Get(p.into()),
                            "]<-" => Notation::Set(p.into()),
                            "<-]" => Notation::Update(p.into()),
                            "..]" => Notation::Cut(p.into()),
                            ".._]" => Notation::Lcut(p.into()),
                            "_..]" => Notation::Rcut(p.into()),
                            _ => Notation::Word(if m == len { s.into() } else { s[..m].into() }),
                        }
                    }
                };
                if m == len {
                    // no appended suffix
                    w
                } else if s.as_bytes()[m] != b'\'' && s.as_bytes()[m] != b'_' {
                    Notation::Word(s.into())
                } else {
                    Notation::Word(
                        format!(
                            "{}",
                            (ocaml_format::Doc::new() as ocaml_format::Doc)
                                .print(print_sn, &w)
                                .atom(&s[m..])
                                .display(&ocaml_format::FormattingOptions::new()),
                        )
                        .into(),
                    )
                }
            }
        }
    }
}

pub static FUNLIT: LazyLock<Attribute> = LazyLock::new(|| create_attribute("funlit".into()));
