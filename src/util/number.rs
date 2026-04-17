//! # General functions for representations of numeric values
//!
//! * [`IntLiteralKind`]
//! * [`IntConstant`]
//! * [`RealValue`]
//! * [`RealLiteralKind`]
//! * [`RealConstant`]
//!
//! ## Pretty-printing with conversion
//!
//! * [`DefaultFormat`]
//! * [`IntegerFormat`]
//! * [`RealFormat`]
//! * [`FracRealFormat`]
//! * [`DelayedFormat`]
//! * [`NumberSupport`]
//! * [`FULL_SUPPORT`]
//! * [`print_int_constant`]
//! * [`print_real_constant`]
//! * [`print_in_base`]

use crate::big_int::*;

use std::{borrow::Cow, sync::LazyLock};

use bumpalo::Bump;
use malachite::{
    Integer,
    base::num::{
        arithmetic::traits::{Abs, Pow, PowerOf2},
        basic::traits::{One, Zero},
        conversion::traits::ToStringBase,
    },
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

fn enforce_same_lifetime<T: for<'a> Fn(&mut ocaml_format::Doc<'a>, &'a Integer)>(closure: T) -> T {
    closure
}

pub enum DefaultOrCustom<T> {
    Default,
    Custom(T),
}

pub enum DefaultOrCustomOrUnsupported<T, U> {
    Default,
    Custom(T),
    Unsupported(U),
}

pub enum CustomOrUnsupported<T, U> {
    Custom(T),
    Unsupported(U),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum IntLiteralKind {
    Unk,
    Dec,
    Hex,
    Oct,
    Bin,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct IntConstant {
    pub kind: IntLiteralKind,
    pub int: Integer,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RealValue {
    pub sig: Integer,
    pub pow2: Integer,
    pub pow5: Integer,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum RealLiteralKind {
    Unk,
    Dec(isize),
    Hex(isize),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RealConstant {
    pub kind: RealLiteralKind,
    pub real: RealValue,
}

/// Prints the value of non-negative `i`
///     in base `radix`. If `digits` is not `None`, it adds leading 0s to have `digits`
///     characters.
pub fn print_in_base(
    radix: u8,
    digits: Option<isize>,
) -> impl Fn(&mut ocaml_format::Doc, &Integer) {
    move |doc, i| {
        assert!(i >= &Integer::ZERO);
        let digits = digits.unwrap_or(1);
        let s = i.to_string_base_upper(radix);
        doc.atom_fn(move |f| write!(f, "{s:0>width$}", width = digits as usize));
    }
}

pub type DefaultFormat = Box<dyn for<'a> Fn(&mut ocaml_format::Doc<'a>, &'a str) + Send + Sync>;

pub type IntegerFormat = Box<dyn for<'a> Fn(&mut ocaml_format::Doc<'a>, &'a Integer) + Send + Sync>;

pub type RealFormat = Box<
    dyn for<'a> Fn(&mut ocaml_format::Doc<'a>, &'a str, &'a str, Option<&'a str>) + Send + Sync,
>;

pub type FracRealFormat = (
    Box<dyn for<'a> Fn(&mut ocaml_format::Doc<'a>, &'a str) + Send + Sync>,
    Box<dyn for<'a> Fn(&mut ocaml_format::Doc<'a>, &'a str, &'a str) + Send + Sync>,
);

pub type DelayedFormat = Box<
    dyn for<'a> Fn(&mut ocaml_format::Doc<'a>, &dyn Fn(&mut ocaml_format::Doc<'a>)) + Send + Sync,
>;

pub struct NumberSupport {
    pub long_int_support: DefaultOrCustom<DefaultFormat>,
    pub negative_int_support: DefaultOrCustom<DelayedFormat>,
    pub dec_int_support: DefaultOrCustomOrUnsupported<IntegerFormat, DefaultFormat>,
    pub hex_int_support: DefaultOrCustomOrUnsupported<IntegerFormat, ()>,
    pub oct_int_support: DefaultOrCustomOrUnsupported<IntegerFormat, ()>,
    pub bin_int_support: DefaultOrCustomOrUnsupported<IntegerFormat, ()>,
    pub negative_real_support: DefaultOrCustom<DelayedFormat>,
    pub dec_real_support: DefaultOrCustomOrUnsupported<RealFormat, ()>,
    pub hex_real_support: DefaultOrCustomOrUnsupported<RealFormat, ()>,
    pub frac_real_support: CustomOrUnsupported<FracRealFormat, DefaultFormat>,
}

pub static FULL_SUPPORT: LazyLock<NumberSupport> = LazyLock::new(|| NumberSupport {
    long_int_support: DefaultOrCustom::Default,
    negative_int_support: DefaultOrCustom::Default,
    dec_int_support: DefaultOrCustomOrUnsupported::Default,
    hex_int_support: DefaultOrCustomOrUnsupported::Default,
    oct_int_support: DefaultOrCustomOrUnsupported::Default,
    bin_int_support: DefaultOrCustomOrUnsupported::Default,
    negative_real_support: DefaultOrCustom::Default,
    dec_real_support: DefaultOrCustomOrUnsupported::Default,
    hex_real_support: DefaultOrCustomOrUnsupported::Default,
    frac_real_support: CustomOrUnsupported::Unsupported(Box::new(|_, _| panic!())),
});

fn check_support<'a, T: ?Sized, U>(
    support: &DefaultOrCustomOrUnsupported<Box<T>, ()>,
    doc: &mut ocaml_format::Doc<'a>,
    do_it: impl FnOnce(&mut ocaml_format::Doc<'a>, &T) -> U,
    default: &T,
    try_next: impl FnOnce(&mut ocaml_format::Doc<'a>) -> U,
) -> U {
    match support {
        DefaultOrCustomOrUnsupported::Unsupported(()) => try_next(doc),
        DefaultOrCustomOrUnsupported::Default => do_it(doc, default),
        DefaultOrCustomOrUnsupported::Custom(f) => do_it(doc, f),
    }
}

fn force_support<T: ?Sized, U>(
    support: &DefaultOrCustom<Box<T>>,
    do_it: impl FnOnce(&T) -> U,
    default: &T,
) -> U {
    match support {
        DefaultOrCustom::Default => do_it(default),
        DefaultOrCustom::Custom(f) => do_it(f),
    }
}

fn force_support_nodef<'a, T, U: ?Sized, V>(
    support: &CustomOrUnsupported<T, Box<U>>,
    doc: &mut ocaml_format::Doc<'a>,
    do_it: impl FnOnce(&mut ocaml_format::Doc<'a>, &T) -> V,
    fallback: impl FnOnce(&mut ocaml_format::Doc<'a>, &U) -> V,
) -> V {
    match support {
        CustomOrUnsupported::Unsupported(f) => fallback(doc, f),
        CustomOrUnsupported::Custom(f) => do_it(doc, f),
    }
}

const SIMPLIFY_MAX_INT: Integer = Integer::const_from_unsigned(2147483646);

fn print_dec_int<'a>(
    support: &NumberSupport,
    doc: &mut ocaml_format::Doc<'a>,
    arena: &'a Bump,
    i: &'a Integer,
) {
    match &support.long_int_support {
        DefaultOrCustom::Custom(f) if i > &SIMPLIFY_MAX_INT => f(doc, arena.alloc(format!("{i}"))),
        _ => match &support.dec_int_support {
            DefaultOrCustomOrUnsupported::Default => {
                doc.atom(i);
            }
            DefaultOrCustomOrUnsupported::Custom(f) => f(doc, i),
            DefaultOrCustomOrUnsupported::Unsupported(f) => f(doc, arena.alloc(format!("{i}"))),
        },
    }
}

fn print_hex_int<'a>(
    support: &NumberSupport,
    doc: &mut ocaml_format::Doc<'a>,
    arena: &'a Bump,
    i: &'a Integer,
) {
    let default = enforce_same_lifetime(|doc, i| {
        assert!(matches!(support.long_int_support, DefaultOrCustom::Default));
        doc.atom("0x").print(print_in_base(16, None), i);
    });
    check_support(
        &support.hex_int_support,
        doc,
        |doc, f| f(doc, i),
        &default,
        |doc| print_dec_int(support, doc, arena, i),
    );
}

fn print_oct_int<'a>(
    support: &NumberSupport,
    doc: &mut ocaml_format::Doc<'a>,
    arena: &'a Bump,
    i: &'a Integer,
) {
    let default = enforce_same_lifetime(|doc, i| {
        assert!(matches!(support.long_int_support, DefaultOrCustom::Default));
        doc.atom("0o").print(print_in_base(8, None), i);
    });
    check_support(
        &support.oct_int_support,
        doc,
        |doc, f| f(doc, i),
        &default,
        |doc| print_hex_int(support, doc, arena, i),
    );
}

fn print_bin_int<'a>(
    support: &NumberSupport,
    doc: &mut ocaml_format::Doc<'a>,
    arena: &'a Bump,
    i: &'a Integer,
) {
    let default = enforce_same_lifetime(|doc, i| {
        assert!(matches!(support.long_int_support, DefaultOrCustom::Default));
        doc.atom("0b").print(print_in_base(2, None), i);
    });
    check_support(
        &support.bin_int_support,
        doc,
        |doc, f| f(doc, i),
        &default,
        |doc| print_hex_int(support, doc, arena, i),
    );
}

fn print_int_literal<'a>(
    support: &NumberSupport,
    doc: &mut ocaml_format::Doc<'a>,
    arena: &'a Bump,
    k: IntLiteralKind,
    i: &'a Integer,
) {
    let p: Box<dyn for<'b> Fn(_, &mut ocaml_format::Doc<'b>, &'b _, &'b _)> = match k {
        IntLiteralKind::Unk => Box::new(print_dec_int),
        IntLiteralKind::Bin => Box::new(print_bin_int),
        IntLiteralKind::Oct => Box::new(print_oct_int),
        IntLiteralKind::Dec => Box::new(print_dec_int),
        IntLiteralKind::Hex => Box::new(print_hex_int),
    };

    if i < &Integer::ZERO {
        fn default<'a>(doc: &mut ocaml_format::Doc<'a>, f: &dyn Fn(&mut ocaml_format::Doc<'a>)) {
            doc.atom("(- ").print_(f).atom(")");
        }

        force_support(
            &support.negative_int_support,
            |f| f(doc, &|doc| p(support, doc, arena, arena.alloc(i.abs()))),
            &default,
        );
    } else {
        p(support, doc, arena, i);
    }
}

pub fn print_int_constant<'a>(
    support: &NumberSupport,
    doc: &mut ocaml_format::Doc<'a>,
    arena: &'a Bump,
    x: &'a IntConstant,
) {
    let IntConstant { kind, int } = x;
    print_int_literal(support, doc, arena, *kind, int);
}

fn print_frac_real<'a>(
    support: &NumberSupport,
    doc: &mut ocaml_format::Doc<'a>,
    arena: &'a Bump,
    RealValue {
        sig: i,
        pow2: p2,
        pow5: p5,
    }: &RealValue,
) {
    let (num, den) = {
        let p2_abs: u64 = p2.unsigned_abs_ref().try_into().unwrap();
        let fact = Integer::power_of_2(p2_abs);
        if p2 < &Integer::ZERO {
            (Cow::Borrowed(i), fact)
        } else {
            (Cow::Owned(i * fact), Integer::ONE)
        }
    };
    let (num, den) = {
        let p5_abs: u64 = p5.unsigned_abs_ref().try_into().unwrap();
        let fact = Integer::from(5).pow(p5_abs);
        if p5 < &Integer::ZERO {
            (num, den * fact)
        } else {
            (Cow::Owned(&*num * fact), den)
        }
    };
    let do_frac = |doc: &mut ocaml_format::Doc<'a>, (no_den, with_den): &FracRealFormat| {
        if den == Integer::ONE {
            no_den(doc, arena.alloc(format!("{num}")));
        } else {
            with_den(
                doc,
                arena.alloc(format!("{num}")),
                arena.alloc(format!("{den}")),
            );
        }
    };
    let fallback = |doc: &mut ocaml_format::Doc<'a>,
                    k: &(
                         dyn for<'b> Fn(&mut ocaml_format::Doc<'b>, &'b str) + Send + Sync + 'static
                     )| k(doc, arena.alloc(format!("{num}_{den}")));
    force_support_nodef(&support.frac_real_support, doc, do_frac, fallback);
}

fn print_dec_real<'a>(
    support: &NumberSupport,
    doc: &mut ocaml_format::Doc<'a>,
    arena: &'a Bump,
    k: &Integer,
    r @ RealValue {
        sig: i,
        pow2: p2,
        pow5: p5,
    }: &RealValue,
) {
    let do_it = |doc: &mut ocaml_format::Doc<'a>,
                 custom: &(
                      dyn for<'b> Fn(&mut ocaml_format::Doc<'b>, &'b str, &'b str, Option<&'b str>)
                          + Send
                          + Sync
                          + 'static
                  )| {
        let e = <&Integer>::min(<&Integer>::min(p2, p5), k);
        let p2_e: u64 = (p2 - e).unsigned_abs_ref().try_into().unwrap();
        let i = i * Integer::power_of_2(p2_e);
        let p5_e: u64 = (p5 - e).unsigned_abs_ref().try_into().unwrap();
        let i = i * Integer::from(5).pow(p5_e);
        let i = format!("{i}");
        let p: usize = (k - e).unsigned_abs_ref().try_into().unwrap();
        let (mut i, n) = {
            let n = i.len();
            if n <= p {
                let width = p + 1;
                (format!("{i:0>width$}"), width)
            } else {
                (i, n)
            }
        };
        let mut f = i.split_off(n - p);
        if f.is_empty() {
            f = "0".into();
        }
        let e = if k == &Integer::ZERO {
            None
        } else {
            Some(arena.alloc(format!("{k}")) as &str)
        };
        custom(doc, arena.alloc(i), arena.alloc(f), e);
    };

    fn default<'a>(doc: &mut ocaml_format::Doc<'a>, i: &'a str, f: &'a str, e: Option<&'a str>) {
        match e {
            None => doc.atom_fn(move |fmt| write!(fmt, "{i}.{f}")),
            Some(e) => doc.atom_fn(move |fmt| write!(fmt, "{i}.{f}e{e}")),
        };
    }

    check_support(&support.dec_real_support, doc, do_it, &default, |doc| {
        print_frac_real(support, doc, arena, r)
    });
}

fn print_hex_real<'a>(
    support: &NumberSupport,
    doc: &mut ocaml_format::Doc<'a>,
    arena: &'a Bump,
    k: &Integer,
    r @ RealValue {
        sig: i,
        pow2: p2,
        pow5: p5,
    }: &RealValue,
) {
    let do_it = |doc: &mut ocaml_format::Doc<'a>,
                 custom: &(
                      dyn for<'b> Fn(&mut ocaml_format::Doc<'b>, &'b str, &'b str, Option<&'b str>)
                          + Send
                          + Sync
                          + 'static
                  )| {
        let e = if p2 < k {
            Cow::Owned(p2 - (p2 - k).euclidean_mod(&Integer::from(4)))
        } else {
            Cow::Borrowed(k)
        };
        assert!(p5 >= &Integer::ZERO);
        let p2_e: u64 = (p2 - &*e).unsigned_abs_ref().try_into().unwrap();
        let i = i * Integer::power_of_2(p2_e);
        let p5_: u64 = p5.unsigned_abs_ref().try_into().unwrap();
        let i = i * Integer::from(5).pow(p5_);
        let i = format!(
            "{}",
            ocaml_format::Doc::new()
                .print(print_in_base(16, None), &i)
                .display(&ocaml_format::FormattingOptions::new()),
        );
        let p: usize = (k - &*e).unsigned_abs_ref().try_into().unwrap();
        assert_eq!(p % 4, 0);
        let p = p / 4;
        let (mut i, n) = {
            let n = i.len();
            if n <= p {
                let width = p + 1;
                (format!("{i:0>width$}"), width)
            } else {
                (i, n)
            }
        };
        let mut f = i.split_off(n - p);
        if f.is_empty() {
            f = "0".into();
        }
        let e = if k == &Integer::ZERO {
            None
        } else {
            Some(arena.alloc(format!("{k}")) as &str)
        };
        custom(doc, arena.alloc(i), arena.alloc(f), e);
    };

    fn default<'a>(doc: &mut ocaml_format::Doc<'a>, i: &'a str, f: &'a str, e: Option<&'a str>) {
        let e = e.unwrap_or("0");
        doc.atom_fn(move |fmt| write!(fmt, "0x{i}.{f}p{e}"));
    }

    check_support(&support.hex_real_support, doc, do_it, &default, |doc| {
        print_dec_real(support, doc, arena, <&Integer>::min(p2, p5), r)
    });
}

fn print_real_constant_<'a>(
    support: &NumberSupport,
    doc: &mut ocaml_format::Doc<'a>,
    arena: &'a Bump,
    RealConstant {
        kind: k,
        real: r @ RealValue {
            pow2: p2, pow5: p5, ..
        },
    }: &RealConstant,
) {
    match *k {
        RealLiteralKind::Dec(e) => print_dec_real(support, doc, arena, &Integer::from(e), r),
        RealLiteralKind::Hex(e) => print_hex_real(support, doc, arena, &Integer::from(e), r),
        RealLiteralKind::Unk => {
            let e = <&Integer>::min(p2, p5);
            let e = if let Ok(ei) = <&Integer as TryInto<isize>>::try_into(e) {
                if (0..=2).contains(&ei) {
                    &Integer::ZERO
                } else {
                    e
                }
            } else {
                e
            };
            print_dec_real(support, doc, arena, e, r);
        }
    }
}

pub fn print_real_constant<'a>(
    support: &NumberSupport,
    doc: &mut ocaml_format::Doc<'a>,
    arena: &'a Bump,
    r: &RealConstant,
) {
    if r.real.sig < Integer::ZERO {
        let r_real = r.real.clone();
        let r = RealConstant {
            real: RealValue {
                sig: -r_real.sig,
                ..r_real
            },
            kind: r.kind,
        };

        fn default<'a>(doc: &mut ocaml_format::Doc<'a>, f: &dyn Fn(&mut ocaml_format::Doc<'a>)) {
            doc.atom("(- ").print_(f).atom(")");
        }

        force_support(
            &support.negative_real_support,
            |f| f(doc, &|doc| print_real_constant_(support, doc, arena, &r)),
            &default,
        );
    } else {
        print_real_constant_(support, doc, arena, r);
    }
}
