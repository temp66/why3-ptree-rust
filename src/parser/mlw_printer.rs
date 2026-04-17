//! # Pretty printing of Why3 parse trees as WhyML source code
//!
//! ## Printers
//!
//! * [`Printers`]
//! * [`pp_pattern`]
//! * [`pp_expr`]
//! * [`pp_term`]
//! * [`pp_pty`]
//! * [`pp_decl`]
//! * [`pp_mlw_file`]
//!
//! ## Markers
//!
//! When [`ptree`] elements are generated (instead of being parsed from a
//! whyml file), locations of typing errors are useless, because they do not
//! correspond to any concrete syntax.
//!
//! Alternatively, we can give every [`ptree`] element a unique location,
//! for example using the function [`mlw_printer::NEXT_POS`]. When a
//! located error is encountered, the function [`with_marker`] can
//! then be used to instruct the mlw-printer to insert a message as a
//! comment just before an expression, term, or pattern with the given
//! location.
//!
//! For example, this can be used to indicate and show a typing error in
//! the printed mlw-file:
//!
//! ```ocaml
//! try
//!   let mm = Typing.type_mlw_file env path filename mlw_file in
//!   (* ... well typed mlw_file ... *)
//! with Loc.Located (loc, e) -> (* A located exception [e] *)
//!   let msg = Format.asprintf "%a" Exn_printer.exn_printer e in
//!   Format.fprintf fmt "%a@."
//!     (Mlw_printer.with_marker ~msg loc Mlw_printer.pp_mlw_file)
//!     mlw_file
//! ```
//!
//! [`ptree`]: crate::ptree
//! [`mlw_printer::NEXT_POS`]: NEXT_POS
//!
//! * [`NEXT_POS`]
//! * [`with_marker`]
//! * [`id_loc`]
//! * [`is_id_loc`]

use crate::{
    constant, debug, decl, dterm, expr, ident, ity, lists, loc, pp, ptree::*, ptree_helpers,
};

use std::{
    borrow::Borrow,
    cell::RefCell,
    collections::HashSet,
    env, iter,
    rc::Rc,
    sync::{
        LazyLock,
        atomic::{AtomicIsize, Ordering},
    },
};

use bumpalo::Bump;
use ocaml_format::*;

fn enforce_same_lifetime<T: ?Sized, U: for<'a> Fn(&mut Doc<'a>, &'a T)>(closure: U) -> U {
    closure
}

static DEBUG_PRINT_IDS: LazyLock<debug::Flag> = LazyLock::new(|| {
    debug::register_flag("mlw_printer_print_ids", {
        let mut doc = DocSync::new();
        doc.atom("Print")
            .space()
            .atom("IDs")
            .space()
            .atom("of")
            .space()
            .atom("unique")
            .space()
            .atom("dummy")
            .space()
            .atom("locations");
        doc
    })
});

/// The `marked` printer potentially adds the marker, the `closed` printer adds
///     parentheses to the potentially marked node
pub struct Printers<'a, T> {
    pub marked: Box<dyn Fn(&mut Doc<'a>, &'a T) + 'a>,
    pub closed: Box<dyn Fn(&mut Doc<'a>, &'a T) + 'a>,
}

thread_local! {
    static MARKER: RefCell<Option<(Rc<str>, loc::Position)>> = const { RefCell::new(None) };
}

const DUMMY_FILENAME: &str = "";

static ID_LOC_COUNTER: AtomicIsize = AtomicIsize::new(0);

/// Create a unique dummy location
pub fn id_loc() -> loc::Position {
    let id_loc_counter = ID_LOC_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    loc::user_position(DUMMY_FILENAME, id_loc_counter, 0, id_loc_counter, 0)
}

pub fn is_id_loc(loc: loc::Position) -> bool {
    let (f, ..) = loc::get(loc);
    &*f == DUMMY_FILENAME
}

static ONLY_IDS: LazyLock<Option<HashSet<isize>>> = LazyLock::new(|| {
    env::var("WHY3MLWPRINTERIDS").ok().and_then(|s| {
        let l: Option<Box<[isize]>> = s.split(',').map(|s| s.parse().ok()).collect();
        l.map(HashSet::from_iter)
    })
});

fn print_only_id(loc: loc::Position) -> bool {
    let (f, l, _, _, _) = loc::get(loc);
    &*f == DUMMY_FILENAME
        && ONLY_IDS
            .as_ref()
            .is_some_and(|only_ids| only_ids.contains(&l))
}

fn pp_loc_id(doc: &mut Doc, loc: loc::Position) {
    if debug::test_flag(*DEBUG_PRINT_IDS) || print_only_id(loc) {
        let (f, bl, bc, _el, ec) = loc::get(loc);
        if &*f == DUMMY_FILENAME && bc == 0 && ec == 0 {
            doc.atom_fn(move |f| write!(f, "(*{bl}*)"));
        }
    }
}

/// Inform a printer to include the message (default: `"XXX"`) as a comment
///    before the expression, term, or pattern with the given location.
pub fn with_marker<'a, T>(
    msg: Option<Rc<str>>,
    loc: loc::Position,
    pp: impl FnOnce(&mut Doc<'a>, T),
    doc: &mut Doc<'a>,
    x: T,
) {
    let msg = msg.unwrap_or("XXX".into());

    MARKER.set(Some((msg, loc)));
    pp(doc, x);
    MARKER.set(None);
}

fn marker(loc: loc::Position) -> Option<Rc<str>> {
    MARKER.with_borrow(|marker| match marker {
        Some((msg, loc_)) if *loc_ == loc => Some(msg.clone()),
        _ => None,
    })
}

fn pp_maybe_marker(doc: &mut Doc, loc: loc::Position) {
    pp_loc_id(doc, loc);
    if let Some(msg) = marker(loc) {
        doc.atom_fn(move |f| write!(f, "(*{msg}*) "));
    }
}

fn pp_maybe_marked<'a, T: Copy>(
    parens: Option<bool>,
    loc: impl FnOnce(T) -> loc::Position,
    pp_raw: impl FnOnce(&mut Doc<'a>, T),
    doc: &mut Doc<'a>,
    x: T,
) {
    let parens = parens.unwrap_or(true);

    let loc = loc(x);
    match marker(loc) {
        Some(msg) => {
            if parens {
                doc.print(pp_loc_id, loc)
                    .atom_fn(move |f| write!(f, "(*{}*)", msg))
                    .space()
                    .atom("(")
                    .print(pp_raw, x)
                    .atom(")");
            } else {
                doc.print(pp_loc_id, loc)
                    .atom_fn(move |f| write!(f, "(*{}*)", msg))
                    .space()
                    .print(pp_raw, x);
            }
        }
        None => {
            doc.print(pp_loc_id, loc).print(pp_raw, x);
        }
    }
}

/// Generate a unique location.
pub static NEXT_POS: LazyLock<Box<dyn Fn() -> loc::Position + Send + Sync>> = LazyLock::new(|| {
    Box::new({
        static COUNTER: AtomicIsize = AtomicIsize::new(0);
        || {
            let counter = COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
            loc::user_position(DUMMY_FILENAME, counter, 0, counter, 0)
        }
    })
});

fn todo<'a>(doc: &mut Doc<'a>, str: &'a str) {
    doc.atom_fn(move |f| write!(f, "__TODO_MLW_PRINTER__ (* {str} *)"));
}

fn pp_sep<'a>(f: &Doc<'a>) -> impl Fn(&mut Doc<'a>) {
    move |doc| {
        doc.extend(f.clone());
    }
}

fn pp_opt<'a, T>(
    prefix: Doc<'a>,
    suffix: Doc<'a>,
    def: Doc<'a>,
    pp: impl FnOnce(&mut Doc<'a>, T),
) -> impl FnOnce(&mut Doc<'a>, Option<T>) {
    |doc, x| match x {
        None => {
            doc.extend(def);
        }
        Some(x) => {
            doc.extend(prefix);
            pp(doc, x);
            doc.extend(suffix);
        }
    }
}

fn pp_print_opt_list<'a, T, U, V: AsRef<[U]> + IntoIterator<Item = T>>(
    prefix: Doc<'a>,
    every: impl Borrow<Doc<'a>>,
    sep: impl Borrow<Doc<'a>>,
    suffix: Doc<'a>,
    def: Doc<'a>,
    mut pp: impl FnMut(&mut Doc<'a>, T),
) -> impl FnOnce(&mut Doc<'a>, V) {
    move |doc, x| match x.as_ref() {
        [] => {
            doc.extend(def);
        }
        _ => {
            let pp = |doc: &mut Doc<'a>, x| {
                doc.extend(every.borrow().clone());
                pp(doc, x);
            };
            doc.extend(prefix)
                .print_iter(Some(pp_sep(sep.borrow())), pp, x)
                .extend(suffix);
        }
    }
}

fn pp_bool<'a>(true_: Option<Doc<'a>>, false_: Option<Doc<'a>>, doc: &mut Doc<'a>, x: bool) {
    if x {
        pp_opt(Doc::default(), Doc::default(), Doc::default(), |doc, f| {
            doc.extend(f);
        })(doc, true_);
    } else {
        pp_opt(Doc::default(), Doc::default(), Doc::default(), |doc, f| {
            doc.extend(f);
        })(doc, false_);
    }
}

fn expr_closed(e: &Expr) -> bool {
    match &e.desc {
        ExprDesc::Ref
        | ExprDesc::True
        | ExprDesc::False
        | ExprDesc::Const(..)
        | ExprDesc::Ident(..)
        | ExprDesc::Tuple(..)
        | ExprDesc::Record(..)
        | ExprDesc::For(..)
        | ExprDesc::While(..)
        | ExprDesc::Assert(..)
        | ExprDesc::Absurd
        | ExprDesc::Scope(..) => true,
        ExprDesc::Idapp(_, expr_list) if expr_list.is_empty() => true,
        ExprDesc::Innfix(..) => true,
        _ => marker(e.loc).is_some(),
    }
}

fn term_closed(t: &Term) -> bool {
    match &t.desc {
        TermDesc::True
        | TermDesc::False
        | TermDesc::Const(..)
        | TermDesc::Ident(..)
        | TermDesc::Update(..)
        | TermDesc::Record(..)
        | TermDesc::Tuple(..)
        | TermDesc::Scope(..) => true,
        TermDesc::Idapp(_, expr_list) if expr_list.is_empty() => true,
        TermDesc::Innfix(..) | TermDesc::Binnop(..) => true,
        _ => marker(t.loc).is_some(),
    }
}

fn pattern_closed(p: &Pattern) -> bool {
    match &p.desc {
        PatDesc::Wild
        | PatDesc::Var(..)
        | PatDesc::Tuple(..)
        | PatDesc::Paren(..)
        | PatDesc::Scope(..) => true,
        PatDesc::App(_, pat_list) if pat_list.is_empty() => true,
        PatDesc::Cast(..) => true,
        _ => marker(p.loc).is_some(),
    }
}

fn pty_closed(t: &Pty) -> bool {
    match t {
        Pty::Tyvar(..) | Pty::Tuple(..) | Pty::Scope(..) | Pty::Paren(..) | Pty::Pure(..) => true,
        Pty::Tyapp(_, pty_list) if pty_list.is_empty() => true,
        _ => false,
    }
}

fn pp_closed<'a, T>(
    is_closed: impl Fn(&T) -> bool,
    pp: impl Fn(&mut Doc<'a>, &'a T),
) -> impl Fn(&mut Doc<'a>, &'a T) {
    move |doc, x| {
        if is_closed(x) {
            pp(doc, x);
        } else {
            doc.hvbox(1, |doc| {
                doc.atom("(").print(&pp, x).atom(")");
            });
        }
    }
}

fn remove_id_attr(s: &str, id: Ident) -> Ident {
    let p = |x: &_| match x {
        Attr::Str(a) => a.0 != s,
        _ => true,
    };
    Ident {
        ats: id.ats.into_iter().filter(p).collect(),
        ..id
    }
}

fn pp_attr_<'a>(doc: &mut Doc<'a>, x: &'a Attr) {
    match x {
        Attr::Str(att) => {
            doc.atom_fn(|f| write!(f, "[@{}]", att.0));
        }
        Attr::Pos(loc) => {
            let (filename, bline, bchar, eline, echar) = loc::get(*loc);
            doc.atom("[#")
                .quoted(filename)
                .atom_fn(move |f| write!(f, " {bline} {bchar} {eline} {echar}]"))
                .print(pp_maybe_marker, *loc);
        }
    }
}

fn pp_id(attr: bool) -> impl for<'a> Fn(&mut Doc<'a>, &'a Ident) {
    move |doc, id| {
        fn pp_decode(doc: &mut Doc, str: &str) {
            use ident::*;

            match sn_decode(str) {
                Notation::Word(s) => doc.atom(s),
                Notation::Infix(s) => doc.atom_fn(move |f| write!(f, "( {s} )")),
                Notation::Prefix(s) => doc.atom_fn(move |f| write!(f, "( {s}_ )")),
                Notation::Tight(s) => doc.atom_fn(move |f| write!(f, "( {s} )")),
                Notation::Get(s) => doc.atom_fn(move |f| write!(f, "( []{s} )")),
                Notation::Set(s) => doc.atom_fn(move |f| write!(f, "( []{s}<- )")),
                Notation::Update(s) => doc.atom_fn(move |f| write!(f, "( [<-]{s} )")),
                Notation::Cut(s) => doc.atom_fn(move |f| write!(f, "( [..]{s} )")),
                Notation::Lcut(s) => doc.atom_fn(move |f| write!(f, "( [.._]{s} )")),
                Notation::Rcut(s) => doc.atom_fn(move |f| write!(f, "( [_..]{s} )")),
            };
        }

        if attr {
            doc.print(pp_maybe_marker, id.loc)
                .print(pp_decode, &id.str)
                .print(
                    pp_print_opt_list(
                        {
                            let mut doc: Doc = Doc::new();
                            doc.atom(" ");
                            doc
                        },
                        &Doc::default(),
                        &Doc::default(),
                        Doc::default(),
                        Doc::default(),
                        pp_attr_,
                    ),
                    &id.ats,
                );
        } else {
            doc.print(pp_maybe_marker, id.loc).print(pp_decode, &id.str);
        }
    }
}

fn pp_qualid(attr: bool) -> impl for<'a> Fn(&mut Doc<'a>, &'a Qualid) {
    move |doc, x| {
        let mut d: Doc = Doc::new();
        pp_id(attr)(&mut d, &x.0[0]);
        for id in &x.0[1..] {
            let mut d_ = Doc::new();
            d_.hbox(|doc| {
                doc.extend(d).atom(".").print(pp_id(attr), id);
            });
            d = d_;
        }
        doc.extend(d);
    }
}

fn pp_true(doc: &mut Doc) {
    doc.atom("true");
}

fn pp_false(doc: &mut Doc) {
    doc.atom("false");
}

fn pp_const<'a>(doc: &mut Doc<'a>, arena: &'a Bump, c: &'a constant::Constant) {
    constant::print_def(doc, arena, c);
}

#[expect(dead_code)]
fn pp_asref<'a>(doc: &mut Doc<'a>, qid: &'a Qualid, attr: bool) {
    doc.hbox(|doc| {
        doc.atom("&").print(pp_qualid(attr), qid);
    });
}

fn pp_idapp<'a, T>(
    pp: &Printers<'a, T>,
    doc: &mut Doc<'a>,
    qid: &'a Qualid,
    xs: &'a [T],
    attr: bool,
) {
    let id = qid.0.last().unwrap();
    if qid.0.len() > 1 {
        match (ident::sn_decode(&id.str), xs) {
            (ident::Notation::Word(s), [x]) if s.as_bytes()[0].is_ascii_lowercase() => {
                doc.hvbox(2, |doc| {
                    doc.print(pp_maybe_marker, id.loc)
                        .print(&pp.closed, x)
                        .atom(".")
                        .print(pp_qualid(attr), qid);
                });
            }
            _ => {
                let pp_args = |doc: &mut Doc<'a>, x| {
                    doc.print_iter(
                        Some(|doc: &mut Doc| {
                            doc.space();
                        }),
                        &pp.closed,
                        x,
                    );
                };
                doc.hvbox(3, |doc| {
                    doc.print(pp_qualid(attr), qid).space().print(pp_args, xs);
                });
            }
        }
    } else {
        match (ident::sn_decode(&id.str), xs) {
            (ident::Notation::Word(s), [x]) if s.as_bytes()[0].is_ascii_lowercase() => {
                doc.hvbox(2, move |doc| {
                    doc.print(pp_maybe_marker, id.loc)
                        .print(&pp.closed, x)
                        .atom_fn(move |f| write!(f, ".{s}"));
                });
            }
            (ident::Notation::Word(s), xs) => {
                let pp_args = |doc: &mut Doc<'a>, x| {
                    doc.print_iter(
                        Some(|doc: &mut Doc| {
                            doc.space();
                        }),
                        &pp.closed,
                        x,
                    );
                };
                doc.hvbox(2, |doc| {
                    doc.print(pp_maybe_marker, id.loc)
                        .atom(s)
                        .space()
                        .print(pp_args, xs);
                });
            }
            (ident::Notation::Infix(s), [x1, x2]) => {
                doc.hvbox(2, move |doc| {
                    doc.print(&pp.closed, x1)
                        .space()
                        .print(pp_maybe_marker, id.loc)
                        .atom_fn(move |f| write!(f, "{s} "))
                        .print(&pp.closed, x2);
                });
            }
            (ident::Notation::Prefix(s), [x]) => {
                doc.hbox(|doc| {
                    doc.print(pp_maybe_marker, id.loc)
                        .atom(s)
                        .space()
                        .print(&pp.closed, x);
                });
            }
            (ident::Notation::Tight(s), [x]) => {
                doc.hbox(|doc| {
                    doc.print(pp_maybe_marker, id.loc)
                        .atom(s)
                        .print(&pp.closed, x);
                });
            }
            (ident::Notation::Get(s), [x1, x2]) => {
                doc.hvbox(1, |doc| {
                    doc.print(&pp.closed, x1)
                        .atom("[")
                        .print(&pp.marked, x2)
                        .atom("]")
                        .print(pp_maybe_marker, id.loc)
                        .atom(s);
                });
            }
            (ident::Notation::Set(s), [x1, x2, x3]) => {
                doc.hvbox(1, |doc| {
                    doc.print(&pp.closed, x1)
                        .atom("[")
                        .print(&pp.marked, x2)
                        .atom("] ")
                        .print(pp_maybe_marker, id.loc)
                        .atom(s)
                        .atom(" <- ")
                        .print(&pp.closed, x3);
                });
            }
            (ident::Notation::Update(s), [x1, x2, x3]) => {
                doc.hbox(|doc| {
                    doc.print(&pp.closed, x1)
                        .atom("[")
                        .print(&pp.marked, x2)
                        .atom(" <- ")
                        .print(&pp.marked, x3)
                        .atom("]")
                        .print(pp_maybe_marker, id.loc)
                        .atom(s);
                });
            }
            (ident::Notation::Cut(s), [x]) => {
                doc.hbox(|doc| {
                    doc.print(&pp.closed, x)
                        .atom("[..]")
                        .print(pp_maybe_marker, id.loc)
                        .atom(s);
                });
            }
            (ident::Notation::Cut(s), [x1, x2, x3]) => {
                doc.hbox(|doc| {
                    doc.print(&pp.closed, x1)
                        .atom("[")
                        .print(&pp.marked, x2)
                        .atom(" .. ")
                        .print(&pp.marked, x3)
                        .atom("]")
                        .print(pp_maybe_marker, id.loc)
                        .atom(s);
                });
            }
            (ident::Notation::Lcut(s), [x1, x2]) => {
                doc.hbox(|doc| {
                    doc.print(&pp.closed, x1)
                        .atom("[.. ")
                        .print(&pp.marked, x2)
                        .atom("]")
                        .print(pp_maybe_marker, id.loc)
                        .atom(s);
                });
            }
            (ident::Notation::Rcut(s), [x1, x2]) => {
                doc.hbox(|doc| {
                    doc.print(&pp.closed, x1)
                        .atom(" [")
                        .print(&pp.marked, x2)
                        .atom(" ..]")
                        .print(pp_maybe_marker, id.loc)
                        .atom(s);
                });
            }
            _ => panic!("pp_idapp"),
        }
    }
}

fn pp_apply<'a, T>(
    split_apply: impl Fn(&T) -> Option<(&T, &T)>,
    pp: &Printers<'a, T>,
    doc: &mut Doc<'a>,
    x1: &'a T,
    x2: &'a T,
) {
    let flatten_applies = |mut sofar: Vec<_>, mut x| loop {
        match split_apply(x) {
            None => {
                sofar.push(x);
                break sofar;
            }
            Some((x1, x2)) => {
                sofar.push(x2);
                x = x1;
            }
        }
    };
    doc.hvbox(2, |doc| {
        doc.print_iter(
            Some(|doc: &mut Doc| {
                doc.space();
            }),
            &pp.closed,
            flatten_applies(vec![x2], x1).into_iter().rev(),
        );
    });
}

fn pp_infix<'a, T>(pp: &Printers<'a, T>, doc: &mut Doc<'a>, x: &'a T, ops: &[(&Ident, &'a T)]) {
    let pp_op = |doc: &mut Doc<'a>, (op, x): &(&Ident, &'a _)| {
        let s = match ident::sn_decode(&op.str) {
            ident::Notation::Infix(s) => s,
            _ => panic!("pp_infix: {}", op.str),
        };
        doc.print(pp_maybe_marker, op.loc)
            .atom(s)
            .space()
            .print(&pp.closed, x);
    };
    doc.print(&pp.closed, x).space().print_iter(
        Some(|doc: &mut Doc| {
            doc.space();
        }),
        pp_op,
        ops,
    );
}

fn pp_innfix<'a, T>(pp: &Printers<'a, T>, doc: &mut Doc<'a>, x1: &'a T, op: &Ident, x2: &'a T) {
    let op = {
        pp_maybe_marker(doc, op.loc);

        use ident::*;

        match sn_decode(&op.str) {
            Notation::Infix(s) => s,
            _ => panic!("pp_innfix: {}", op.str),
        }
    };
    doc.hvbox(3, move |doc| {
        doc.atom("(")
            .print(&pp.closed, x1)
            .space()
            .atom_fn(move |f| write!(f, "{op} "))
            .print(&pp.closed, x2)
            .atom(")");
    });
}

fn pp_not<'a, T>(pp: &Printers<'a, T>, doc: &mut Doc<'a>, x: &'a T) {
    doc.hbox(|doc| {
        doc.atom("not").space().print(&pp.closed, x);
    });
}

fn pp_scope<'a, T>(pp: &Printers<'a, T>, doc: &mut Doc<'a>, qid: &'a Qualid, x: &'a T, attr: bool) {
    doc.hvbox(2, |doc| {
        doc.print(pp_qualid(attr), qid)
            .atom(".(")
            .print(&pp.marked, x)
            .atom(")");
    });
}

fn pp_tuple<'a, T>(pp: &Printers<'a, T>, doc: &mut Doc<'a>, xs: &'a [T]) {
    let pp_xs = |doc: &mut Doc<'a>, x| {
        doc.print_iter(
            Some(|doc: &mut Doc| {
                doc.atom(",").space();
            }),
            &pp.closed,
            x,
        );
    };
    doc.hvbox(1, |doc| {
        doc.atom("(").print(pp_xs, xs).atom(")");
    });
}

fn pp_field<'a, T>(pp: &Printers<'a, T>, attr: bool) -> impl Fn(&mut Doc<'a>, &'a (Qualid, T)) {
    move |doc, (qid, x)| {
        doc.hvbox(2, |doc| {
            doc.print(pp_qualid(attr), qid)
                .atom(" =")
                .space()
                .print(&pp.closed, x);
        });
    }
}

fn pp_fields<'a, T>(pp: &Printers<'a, T>, attr: bool) -> impl Fn(&mut Doc<'a>, &'a [(Qualid, T)]) {
    move |doc, x| {
        doc.print_iter(
            Some(|doc: &mut Doc| {
                doc.atom(" ;").space();
            }),
            pp_field(pp, attr),
            x,
        );
    }
}

fn pp_record<'a, T>(pp: &Printers<'a, T>, doc: &mut Doc<'a>, fs: &'a [(Qualid, T)], attr: bool) {
    doc.sbox(0, |doc| {
        doc.hvbox(2, |doc| {
            doc.atom("{ ").print(pp_fields(pp, attr), fs);
        })
        .atom(" }");
    });
}

fn pp_update<'a, T>(
    pp: &Printers<'a, T>,
    doc: &mut Doc<'a>,
    x: &'a T,
    fs: &'a [(Qualid, T)],
    attr: bool,
) {
    doc.hvbox(2, |doc| {
        doc.atom("{ ")
            .print(&pp.closed, x)
            .atom(" with")
            .space()
            .print(pp_fields(pp, attr), fs)
            .atom(" }");
    });
}

/// Printer for types
pub fn pp_pty<'a>(attr: bool) -> Printers<'a, Pty> {
    fn raw(attr: bool) -> impl for<'a> Fn(&mut Doc<'a>, &'a Pty) {
        move |doc, x| match x {
            Pty::Tyvar(id) => {
                doc.atom("'").print(pp_id(attr), id);
            }
            Pty::Tyapp(qid, ptys) if ptys.is_empty() => {
                pp_qualid(attr)(doc, qid);
            }
            Pty::Tyapp(qid, ptys) => {
                let pp_ptys = enforce_same_lifetime(|doc, x| {
                    doc.print_iter(
                        Some(|doc: &mut Doc| {
                            doc.atom(" ");
                        }),
                        pp_pty(attr).closed,
                        x,
                    );
                });
                doc.hvbox(2, |doc| {
                    doc.print(pp_qualid(attr), qid).space().print(pp_ptys, ptys);
                });
            }
            Pty::Tuple(ptys) => pp_tuple(&pp_pty(attr), doc, ptys),
            Pty::Ref(_ptys) => {
                panic!("mlw_printer::pp_pty: Pty::Ref (must be handled by caller of pp_pty)");
            }
            Pty::Arrow(pty1, pty2) => {
                doc.hvbox(2, |doc| {
                    doc.print(pp_pty(attr).closed, pty1)
                        .atom(" ->")
                        .space()
                        .print(pp_pty(attr).closed, pty2);
                });
            }
            Pty::Scope(qid, pty) => pp_scope(&pp_pty(attr), doc, qid, pty, attr),
            Pty::Paren(pty) => {
                doc.atom("(").print(pp_pty(attr).marked, pty).atom(")");
            }
            Pty::Pure(pty) => {
                doc.atom("{").print(pp_pty(attr).marked, pty).atom("}");
            }
        }
    }

    fn closed<'a>(attr: bool) -> impl Fn(&mut Doc<'a>, &'a Pty) {
        pp_closed(pty_closed, raw(attr))
    }

    Printers {
        marked: Box::new(raw(attr)),
        closed: Box::new(closed(attr)),
    }
}

fn pp_opt_pty<'a>(attr: bool) -> impl FnOnce(&mut Doc<'a>, Option<&'a Pty>) {
    pp_opt(
        {
            let mut doc: Doc = Doc::new();
            doc.atom(" : ");
            doc
        },
        Doc::default(),
        Doc::default(),
        pp_pty(attr).marked,
    )
}

fn pp_ghost(doc: &mut Doc, ghost: bool) {
    if ghost {
        doc.atom("ghost ");
    }
}

fn pp_mutable(doc: &mut Doc, mutable_: bool) {
    if mutable_ {
        doc.atom("mutable ");
    }
}

fn pp_kind(doc: &mut Doc, x: expr::RsKind) {
    match x {
        expr::RsKind::None => (),
        expr::RsKind::Func => {
            doc.atom("function ");
        }
        expr::RsKind::Pred => {
            doc.atom("predicate ");
        }
        expr::RsKind::Lemma => {
            doc.atom("lemma ");
        }
        // assert false? does not occur in parser
        expr::RsKind::Local => todo(doc, "RKLOCAL"),
    }
}

fn opt_ref_pty(x: &Pty) -> (&'static str, &Pty) {
    match x {
        Pty::Ref(ptys) if ptys.len() == 1 => {
            let pty = &ptys[0];
            ("ref ", pty)
        }
        _ => ("", x),
    }
}

fn pp_binder<'a>(arena: &'a Bump, attr: bool) -> impl Fn(&mut Doc<'a>, &'a Binder) {
    move |doc, Binder(loc, opt_id, ghost, opt_pty)| {
        let (opt_ref, opt_pty) = match opt_pty.as_ref().map(opt_ref_pty) {
            Some((r#ref, pty)) => (r#ref, Some(pty)),
            None => ("", None),
        };
        let pp_opt_id = enforce_same_lifetime(|doc, opt_id: &_| match opt_id {
            None => {
                doc.atom("_");
            }
            Some(id) => pp_id(attr)(doc, id),
        });
        if *ghost || opt_pty.is_some() {
            let opt_id = arena.alloc(
                opt_id
                    .as_ref()
                    .map(|id| remove_id_attr("mlw:reference_var", id.clone())),
            );
            doc.print(pp_maybe_marker, *loc)
                .atom("(")
                .print(pp_ghost, *ghost)
                .atom(opt_ref)
                .print(pp_opt_id, opt_id)
                .print(pp_opt_pty(attr), opt_pty)
                .atom(")");
        } else {
            doc.print(pp_maybe_marker, *loc).print(pp_opt_id, opt_id);
        }
    }
}

fn pp_binders<'a>(arena: &'a Bump, attr: bool) -> impl FnOnce(&mut Doc<'a>, &'a [Binder]) {
    pp_print_opt_list(
        {
            let mut doc: Doc = Doc::new();
            doc.atom(" ");
            doc
        },
        Doc::default(),
        Doc::default(),
        Doc::default(),
        Doc::default(),
        pp_binder(arena, attr),
    )
}

fn pp_comma_binder(attr: bool) -> impl for<'a> Fn(&mut Doc<'a>, &'a Binder) {
    move |doc, Binder(loc, opt_id, ghost, opt_pty)| {
        let (opt_ref, opt_pty) = match opt_pty.as_ref().map(opt_ref_pty) {
            Some((r#ref, pty)) => (r#ref, Some(pty)),
            None => ("", None),
        };
        doc.print(pp_maybe_marker, *loc)
            .print(pp_ghost, *ghost)
            .atom(opt_ref)
            .print(
                pp_opt(
                    Doc::default(),
                    Doc::default(),
                    {
                        let mut doc: Doc = Doc::new();
                        doc.atom("_");
                        doc
                    },
                    pp_id(attr),
                ),
                opt_id.as_ref(),
            )
            .print(
                pp_opt(
                    {
                        let mut doc: Doc = Doc::new();
                        doc.atom(" : ");
                        doc
                    },
                    Doc::default(),
                    Doc::default(),
                    pp_pty(attr).marked,
                ),
                opt_pty,
            );
    }
}

fn pp_comma_binders<'a>(attr: bool) -> impl FnOnce(&mut Doc<'a>, &'a [Binder]) {
    pp_print_opt_list(
        {
            let mut doc: Doc = Doc::new();
            doc.atom(" ");
            doc
        },
        Doc::default(),
        {
            let mut doc: Doc = Doc::new();
            doc.atom(", ");
            doc
        },
        Doc::default(),
        Doc::default(),
        pp_comma_binder(attr),
    )
}

fn pp_param<'a>(arena: &'a Bump, attr: bool) -> impl Fn(&mut Doc<'a>, &'a Param) {
    move |doc, Param(loc, opt_id, ghost, pty)| {
        let (opt_ref, pty, opt_id) = match pty {
            Pty::Ref(ptys) if ptys.len() == 1 => {
                let pty = &ptys[0];
                (
                    "ref ",
                    pty,
                    arena.alloc(
                        opt_id
                            .as_ref()
                            .map(|id| remove_id_attr("mlw:reference_var", id.clone())),
                    ) as &_,
                )
            }
            _ => ("", pty, opt_id),
        };
        if *ghost || opt_id.is_some() || !opt_ref.is_empty() {
            let pp_id = enforce_same_lifetime(|doc, id| {
                doc.print(pp_id(attr), id).atom(":");
            });
            doc.print(pp_maybe_marker, *loc)
                .atom("(")
                .print(pp_ghost, *ghost)
                .atom(opt_ref)
                .print(pp::print_option(pp_id), opt_id.as_ref())
                .atom(" ")
                .print(pp_pty(attr).marked, pty);
        }
    }
}

fn pp_params<'a>(arena: &'a Bump, attr: bool) -> impl FnOnce(&mut Doc<'a>, &'a [Param]) {
    pp_print_opt_list(
        {
            let mut doc: Doc = Doc::new();
            doc.atom(" ");
            doc
        },
        Doc::default(),
        Doc::default(),
        Doc::default(),
        Doc::default(),
        pp_param(arena, attr),
    )
}

fn pp_if<'a, T>(pp: &Printers<'a, T>, doc: &mut Doc<'a>, x1: &'a T, x2: &'a T, x3: &'a T) {
    doc.vbox(0, |doc| {
        doc.hvbox(2, |doc| {
            doc.atom("if ")
                .print(&pp.closed, x1)
                .atom(" then")
                .space()
                .print(&pp.closed, x2);
        })
        .space()
        .hvbox(2, |doc| {
            doc.atom("else").space().print(&pp.closed, x3);
        });
    });
}

fn pp_cast<'a, T>(pp: &Printers<'a, T>, doc: &mut Doc<'a>, x: &'a T, pty: &'a Pty, attr: bool) {
    doc.hvbox(2, |doc| {
        doc.print(&pp.closed, x)
            .atom(" :")
            .space()
            .print(pp_pty(attr).closed, pty);
    });
}

fn pp_attr<'a, T>(pp: impl FnOnce(&mut Doc<'a>, T), doc: &mut Doc<'a>, attr: &'a Attr, x: T) {
    doc.sbox(0, |doc| {
        doc.print(pp_attr_, attr).print(pp, x);
    });
}

fn pp_pty_mask(attr: bool) -> impl for<'a> Fn(&mut Doc<'a>, (&'a Pty, &'a ity::Mask)) {
    move |doc, x| match x {
        (Pty::Tuple(ptys), ity::Mask::Visible) if ptys.is_empty() => (),
        (pty, ity::Mask::Visible) => {
            let (opt_ref, pty) = opt_ref_pty(pty);
            doc.atom(opt_ref).print(pp_pty(attr).marked, pty);
        }
        (pty, ity::Mask::Ghost) => {
            doc.atom("ghost ").print(pp_pty(attr).closed, pty);
        }
        (Pty::Tuple(ptys), ity::Mask::Tuple(ms)) => {
            doc.atom("(")
                .print_iter(Some(pp::comma), pp_pty_mask(attr), iter::zip(ptys, ms))
                .atom(")");
        }
        (_, ity::Mask::Tuple(..)) => panic!(),
    }
}

fn pp_pty_pat_mask<'a>(
    attr: bool,
    closed: bool,
) -> impl Fn(&mut Doc<'a>, (&'a Pty, (&'a Pattern, &'a ity::Mask))) {
    fn pp_vis_ghost(doc: &mut Doc, x: &ity::Mask) {
        match x {
            ity::Mask::Visible => (),
            ity::Mask::Ghost => {
                doc.atom("ghost ");
            }
            ity::Mask::Tuple(..) => {
                doc.atom("TUPLE??");
            }
        }
    }

    let pp_aux = move |doc: &mut Doc<'a>, x: (&'a _, (&'a _, &'a _))| match x {
        (
            Pty::Tuple(ptys),
            (
                Pattern {
                    desc: PatDesc::Tuple(ps),
                    ..
                },
                ity::Mask::Tuple(ms),
            ),
        ) => {
            doc.atom("(")
                .print_iter(
                    Some(pp::comma),
                    pp_pty_pat_mask(attr, false),
                    iter::zip(ptys, iter::zip(ps, ms)),
                )
                .atom(")");
        }
        (
            Pty::Tuple(ptys),
            (
                Pattern {
                    desc: PatDesc::Wild,
                    ..
                },
                ity::Mask::Tuple(ms),
            ),
        ) => {
            doc.atom("(")
                .print_iter(Some(pp::comma), pp_pty_mask(attr), iter::zip(ptys, ms))
                .atom(")");
        }
        (
            pty,
            (
                Pattern {
                    desc: PatDesc::Wild,
                    ..
                },
                m,
            ),
        ) => {
            let (opt_ref, pty) = opt_ref_pty(pty);
            doc.print(pp_vis_ghost, m)
                .atom(opt_ref)
                .print(pp_pty(attr).closed, pty);
        }
        (
            pty,
            (
                Pattern {
                    desc: PatDesc::Var(id),
                    ..
                },
                m,
            ),
        ) => {
            // (ghost) x: t
            let (opt_ref, pty) = opt_ref_pty(pty);
            let pp = |doc: &mut Doc<'a>| {
                doc.print(pp_vis_ghost, m)
                    .atom(opt_ref)
                    .print(pp_id(attr), id)
                    .atom(" : ")
                    .print(pp_pty(attr).marked, pty);
            };
            if closed {
                doc.atom("(").print_(pp).atom(")");
            } else {
                doc.print_(pp);
            }
        }
        _ => panic!(),
    };
    move |doc, (pty, (pat, m))| {
        doc.sbox(0, |doc| {
            doc.print(pp_maybe_marker, pat.loc)
                .print(pp_aux, (pty, (pat, m)));
        });
    }
}

fn pp_opt_result(
    attr: bool,
) -> impl for<'a> Fn(&mut Doc<'a>, (&'a Option<Pty>, &'a Pattern, &'a ity::Mask)) {
    move |doc, (opt_pty, p, m)| {
        let Some(pty) = opt_pty else {
            return;
        };
        doc.atom(" : ")
            .print(pp_pty_pat_mask(attr, true), (pty, (p, m)));
    }
}

fn pp_exn(attr: bool) -> impl for<'a> Fn(&mut Doc<'a>, (&'a Ident, &'a Pty, &'a ity::Mask)) {
    move |doc, (id, pty, m)| {
        let pp_space = |doc: &mut Doc| {
            if let Pty::Tuple(ptys) = pty
                && ptys.is_empty()
            {
                return;
            }
            doc.atom(" ");
        };
        doc.hbox(|doc| {
            doc.atom("exception ")
                .print(pp_id(attr), id)
                .print_(pp_space)
                .print(pp_pty_mask(attr), (pty, m));
        });
    }
}

fn remove_witness_existence(pre: Box<[Pre]>) -> Box<[Pre]> {
    fn not_witness_existence(x: &Pre) -> bool {
        if let Term {
            desc: TermDesc::Attr(Attr::Str(ident::Attribute(string)), _),
            ..
        } = x
            && string == "expl:witness existence"
        {
            false
        } else {
            true
        }
    }

    pre.into_iter().filter(not_witness_existence).collect()
}

fn pp_let<'a, T>(
    pp: &Printers<'a, T>,
    is_ref: impl Fn(&T) -> Option<(loc::Position, &T)>,
    arena: &'a Bump,
    attr: bool,
) -> impl Fn(&mut Doc<'a>, (&'a Ident, bool, expr::RsKind, &'a T)) {
    move |doc, (id, ghost, kind, x)| match is_ref(x) {
        Some((loc, x))
            if id.ats.iter().any(|x| {
                if let Attr::Str(a) = x
                    && a.0 == "mlw:reference_var"
                {
                    true
                } else {
                    false
                }
            }) =>
        {
            doc.hvbox(2, |doc| {
                doc.atom("let ")
                    .print(pp_ghost, ghost)
                    .atom("ref ")
                    .print(pp_kind, kind)
                    .print(
                        pp_id(attr),
                        arena.alloc(remove_id_attr("mlw:reference_var", id.clone())),
                    )
                    .atom(" =")
                    .space()
                    .print(pp_maybe_marker, loc)
                    .print(&pp.marked, x);
            });
        }
        _ => {
            doc.hvbox(2, |doc| {
                doc.atom("let ")
                    .print(pp_ghost, ghost)
                    .print(pp_kind, kind)
                    .print(pp_id(attr), id)
                    .atom(" =")
                    .space()
                    .print(&pp.marked, x);
            });
        }
    }
}

fn is_ref_expr(x: &Expr) -> Option<(loc::Position, &Expr)> {
    if let Expr {
        loc,
        desc: ExprDesc::Apply(e1, e2),
    } = x
        && let Expr {
            desc: ExprDesc::Ref,
            ..
        } = e1.as_ref()
    {
        Some((*loc, e2))
    } else {
        None
    }
}

fn is_ref_pattern(_: &Term) -> Option<(loc::Position, &Term)> {
    None
}

fn remove_term_attr<'a>(s: &str, t: &'a Term) -> &'a Term {
    if let TermDesc::Attr(Attr::Str(attr), t) = &t.desc
        && attr.0 == s
    {
        t
    } else {
        t
    }
}

fn pp_clone_subst(attr: bool) -> impl for<'a> Fn(&mut Doc<'a>, &'a CloneSubst) {
    move |doc, x| match x {
        CloneSubst::Axiom(qid) => {
            doc.atom("axiom ").print(pp_qualid(attr), qid);
        }
        CloneSubst::Tsym(qid, args, pty) => {
            let pp_args = pp_print_opt_list(
                Doc::default(),
                {
                    let mut doc: Doc = Doc::new();
                    doc.atom(" '");
                    doc
                },
                Doc::default(),
                Doc::default(),
                Doc::default(),
                pp_id(attr),
            );
            doc.atom("type ")
                .print(pp_qualid(attr), qid)
                .print(pp_args, args)
                .atom(" = ")
                .print(pp_pty(attr).marked, pty);
        }
        CloneSubst::Fsym(qid, qid_) => {
            doc.atom("function ")
                .print(pp_qualid(attr), qid)
                .atom(" = ")
                .print(pp_qualid(attr), qid_);
        }
        CloneSubst::Psym(qid, qid_) => {
            doc.atom("predicate ")
                .print(pp_qualid(attr), qid)
                .atom(" = ")
                .print(pp_qualid(attr), qid_);
        }
        CloneSubst::Vsym(qid, qid_) => {
            doc.atom("val ")
                .print(pp_qualid(attr), qid)
                .atom(" = ")
                .print(pp_qualid(attr), qid_);
        }
        CloneSubst::Xsym(qid, qid_) => {
            if qid == qid_ {
                doc.atom("exception ").print(pp_qualid(attr), qid);
            } else {
                doc.atom("exception ")
                    .print(pp_qualid(attr), qid)
                    .atom(" = ")
                    .print(pp_qualid(attr), qid_);
            }
        }
        CloneSubst::Prop(decl::PropKind::Axiom) => {
            doc.atom("axiom .");
        }
        CloneSubst::Prop(decl::PropKind::Lemma) => {
            doc.atom("lemma .");
        }
        CloneSubst::Prop(decl::PropKind::Goal) => {
            doc.atom("goal .");
        }
        CloneSubst::Lemma(qid) => {
            doc.atom("lemma ").print(pp_qualid(attr), qid);
        }
        CloneSubst::Goal(qid) => {
            doc.atom("goal ").print(pp_qualid(attr), qid);
        }
    }
}

fn pp_substs<'a>(attr: bool) -> impl FnOnce(&mut Doc<'a>, &'a [CloneSubst]) {
    pp_print_opt_list(
        {
            let mut doc: Doc = Doc::new();
            doc.atom(" with").space();
            doc
        },
        Doc::default(),
        {
            let mut doc: Doc = Doc::new();
            doc.atom(",").space();
            doc
        },
        Doc::default(),
        Doc::default(),
        pp_clone_subst(attr),
    )
}

fn pp_import(doc: &mut Doc, import: bool) {
    if import {
        doc.atom("import ");
    }
}

fn pp_match<'a, T>(
    pp: &Printers<'a, T>,
    pp_pattern: &Printers<'a, Pattern>,
    doc: &mut Doc<'a>,
    x: &'a T,
    cases: &'a [(Pattern, T)],
    xcases: &'a [(Qualid, Option<Pattern>, T)],
    attr: bool,
) {
    let pp_reg_branch = |doc: &mut Doc<'a>, (p, x): &'a _| {
        doc.hvbox(2, |doc| {
            doc.print(&pp_pattern.marked, p)
                .atom(" ->")
                .space()
                .print(&pp.marked, x);
        });
    };
    let pp_exn_branch = |doc: &mut Doc<'a>, (qid, p_opt, x): &'a (_, Option<_>, _)| {
        doc.hvbox(2, |doc| {
            doc.print(pp_qualid(attr), qid)
                .print(
                    pp_opt(
                        Doc::default(),
                        {
                            let mut doc: Doc = Doc::new();
                            doc.atom(" ");
                            doc
                        },
                        Doc::default(),
                        &pp_pattern.marked,
                    ),
                    p_opt.as_ref(),
                )
                .atom(" -> ")
                .print(&pp.marked, x);
        });
    };
    doc.vbox(0, |doc| {
        doc.hvbox(2, |doc| {
            doc.atom("match ").print(&pp.marked, x).atom(" with");
        })
        .print(
            pp_print_opt_list(
                {
                    let mut doc: Doc = Doc::new();
                    doc.space().atom("| ");
                    doc
                },
                &Doc::default(),
                &{
                    let mut doc: Doc = Doc::new();
                    doc.space().atom("| ");
                    doc
                },
                Doc::default(),
                Doc::default(),
                pp_reg_branch,
            ),
            cases,
        )
        .print(
            pp_print_opt_list(
                {
                    let mut doc: Doc = Doc::new();
                    doc.space().atom("| exception ");
                    doc
                },
                &Doc::default(),
                &{
                    let mut doc: Doc = Doc::new();
                    doc.space().atom("| exception ");
                    doc
                },
                Doc::default(),
                Doc::default(),
                pp_exn_branch,
            ),
            xcases,
        )
        .space()
        .atom("end");
    });
}

fn pp_partial(doc: &mut Doc, x: bool) {
    pp_bool(
        Some({
            let mut doc: Doc = Doc::new();
            doc.atom("partial ");
            doc
        }),
        Some({
            let mut doc: Doc = Doc::new();
            doc.atom("partial ");
            doc
        }),
        doc,
        x,
    );
}

fn term_hyp_name(x: &Term) -> (Option<loc::Position>, String, &Term) {
    if let Term {
        loc,
        desc: TermDesc::Attr(Attr::Str(ident::Attribute(attr)), t),
    } = x
        && let Some(attr_) = attr.strip_prefix("hyp_name:")
    {
        (Some(*loc), format!(" {}", attr_), t)
    } else {
        (None, String::new(), x)
    }
}

fn attr_equals(at1: &Attr, at2: &Attr) -> bool {
    match (at1, at2) {
        (Attr::Str(at1), Attr::Str(at2)) => at1.0 == at2.0,
        (Attr::Pos(loc1), Attr::Pos(loc2)) => loc::equal(*loc1, *loc2),
        _ => false,
    }
}

fn ident_equals(id1: &Ident, id2: &Ident) -> bool {
    id1.str == id2.str && lists::equal(attr_equals, &id1.ats, &id2.ats)
}

fn qualid_equals(qid1: &Qualid, qid2: &Qualid) -> bool {
    lists::equal(ident_equals, &qid1.0, &qid2.0)
}

fn pty_equals(_: &Pty, _: &Pty) -> bool {
    // TODO
    true
}

fn pat_equals(p1: &Pattern, p2: &Pattern) -> bool {
    match (&p1.desc, &p2.desc) {
        (PatDesc::Wild, PatDesc::Wild) => true,
        (PatDesc::Var(id1), PatDesc::Var(id2)) => ident_equals(id1, id2),
        (PatDesc::App(qid1, pats1), PatDesc::App(qid2, pats2)) => {
            qualid_equals(qid1, qid2) && lists::equal(pat_equals, pats1, pats2)
        }
        (PatDesc::Rec(l1), PatDesc::Rec(l2)) => {
            fn equals((qid1, pat1): &(Qualid, Pattern), (qid2, pat2): &(Qualid, Pattern)) -> bool {
                qualid_equals(qid1, qid2) && pat_equals(pat1, pat2)
            }

            lists::equal(equals, l1, l2)
        }
        (PatDesc::Tuple(pats1), PatDesc::Tuple(pats2)) => lists::equal(pat_equals, pats1, pats2),
        (PatDesc::As(p1, id1, g1), PatDesc::As(p2, id2, g2)) => {
            pat_equals(p1, p2) && ident_equals(id1, id2) && g1 == g2
        }
        (PatDesc::Or(p1, p1_), PatDesc::Or(p2, p2_)) => pat_equals(p1, p2) && pat_equals(p1_, p2_),
        (PatDesc::Cast(p1, pty1), PatDesc::Cast(p2, pty2)) => {
            pat_equals(p1, p2) && pty_equals(pty1, pty2)
        }
        (PatDesc::Scope(qid1, p1), PatDesc::Scope(qid2, p2)) => {
            qualid_equals(qid1, qid2) && pat_equals(p1, p2)
        }
        (PatDesc::Paren(p1), PatDesc::Paren(p2)) => pat_equals(p1, p2),
        (PatDesc::Ghost(p1), PatDesc::Ghost(p2)) => pat_equals(p1, p2),
        _ => false,
    }
}

fn pp_fun<'a>(
    pp: &Printers<'a, Expr>,
    doc: &mut Doc<'a>,
    arena: &'a Bump,
    x: (
        &'a [Binder],
        &'a Option<Pty>,
        &'a Pattern,
        &'a ity::Mask,
        &'a Spec,
        &'a Expr,
    ),
    attr: bool,
) {
    match x {
        ([], None, pat, ity::Mask::Visible, spec, e) if pat.desc == PatDesc::Wild => {
            doc.hvbox(0, |doc| {
                doc.vbox(2, |doc| {
                    doc.print(pp_maybe_marker, pat.loc)
                        .atom("begin")
                        .space()
                        .print(pp_spec(pat, arena, attr), spec)
                        .print(&pp.marked, e);
                })
                .space()
                .atom("end");
            });
        }
        (binders, opt_pty, pat, mask, spec, e) => {
            doc.hvbox(2, |doc| {
                doc.atom("fun ")
                    .print(pp_binders(arena, attr), binders)
                    .print(pp_opt_result(attr), (opt_pty, pat, mask))
                    .print(pp_spec(pat, arena, attr), spec)
                    .atom(" ->")
                    .space()
                    .sbox(0, |doc| {
                        doc.print(&pp.marked, e);
                    });
            });
        }
    }
}

fn pp_let_fun<'a>(
    pp: &Printers<'a, Expr>,
    arena: &'a Bump,
    attr: bool,
) -> impl Fn(
    &mut Doc<'a>,
    (
        loc::Position,
        &'a Ident,
        bool,
        expr::RsKind,
        (
            &'a [Binder],
            &'a Option<Pty>,
            &'a Pattern,
            &'a ity::Mask,
            &Spec,
            &'a Expr,
        ),
    ),
) {
    move |doc, (loc, id, ghost, kind, (binders, opt_pty, pat, mask, spec, x))| {
        match (binders, opt_pty, mask, pat) {
            (
                [],
                None,
                ity::Mask::Visible,
                Pattern {
                    desc: PatDesc::Wild,
                    ..
                },
            ) => doc.hvbox(0, |doc| {
                doc.vbox(2, |doc| {
                    doc.atom("let ")
                        .print(pp_ghost, ghost)
                        .print(pp_partial, spec.partial)
                        .print(pp_kind, kind)
                        .print(pp_id(attr), id)
                        .atom(" = ")
                        .print(pp_maybe_marker, loc)
                        .print(pp_maybe_marker, pat.loc)
                        .atom("begin")
                        .space()
                        .print(
                            pp_spec(pat, arena, attr),
                            arena.alloc(Spec {
                                partial: false,
                                ..spec.clone()
                            }),
                        )
                        .space()
                        .print(&pp.marked, x);
                })
                .space()
                .atom("end");
            }),
            _ => doc.sbox(0, |doc| {
                doc.vbox(2, |doc| {
                    doc.atom("let ")
                        .print(pp_ghost, ghost)
                        .print(pp_partial, spec.partial)
                        .print(pp_kind, kind)
                        .print(pp_id(attr), id)
                        .print(pp_binders(arena, attr), binders)
                        .print(pp_opt_result(attr), (opt_pty, pat, mask))
                        .print(pp_maybe_marker, loc)
                        .print(
                            pp_spec(pat, arena, attr),
                            arena.alloc(Spec {
                                partial: false,
                                ..spec.clone()
                            }),
                        );
                })
                .newline()
                .vbox(2, |doc| {
                    doc.atom("= ").print(&pp.marked, x);
                });
            }),
        };
    }
}

fn pp_let_any<'a>(
    arena: &'a Bump,
    attr: bool,
) -> impl Fn(
    &mut Doc<'a>,
    (
        loc::Position,
        &'a Ident,
        bool,
        expr::RsKind,
        (
            &'a [Param],
            expr::RsKind,
            &'a Option<Pty>,
            &'a Pattern,
            &'a ity::Mask,
            &Spec,
        ),
    ),
) {
    move |doc, (loc, id, ghost, kind, (params, kind_, opt_pty, pat, mask, spec))| {
        if kind_ != expr::RsKind::None {
            // Concrete syntax?
            todo(doc, "LET-ANY kind<>RKnone");
            return;
        }
        match (opt_pty, &pat.desc, mask, &*spec.pre) {
            (
                Some(pty),
                PatDesc::Wild,
                ity::Mask::Visible,
                [
                    pre @ ..,
                    Term {
                        desc: TermDesc::Attr(Attr::Str(ident::Attribute(string)), _),
                        ..
                    },
                ],
            ) if string == "expl:witness existence" => {
                doc.hvbox(0, |doc| {
                    doc.hvbox(2, |doc| {
                        doc.atom("let ")
                            .print(pp_maybe_marker, loc)
                            .print(pp_ghost, ghost)
                            .print(pp_partial, spec.partial)
                            .print(pp_kind, kind)
                            .print(pp_id(attr), id)
                            .print(pp_params(arena, attr), params)
                            .space();
                    })
                    .atom("=")
                    .space()
                    .hvbox(2, |doc| {
                        doc.atom("any ")
                            .print(pp_pty(attr).closed, pty)
                            .atom(" ")
                            .print(
                                pp_spec(pat, arena, attr),
                                arena.alloc(Spec {
                                    pre: pre.into(),
                                    ..spec.clone()
                                }),
                            );
                    });
                });
            }
            _ => {
                doc.vbox(2, |doc| {
                    doc.atom("val ")
                        .print(pp_maybe_marker, loc)
                        .print(pp_ghost, ghost)
                        .print(pp_partial, spec.partial)
                        .print(pp_kind, kind)
                        .print(pp_id(attr), id)
                        .print(pp_params(arena, attr), params)
                        .print(pp_opt_result(attr), (opt_pty, pat, mask))
                        .print(
                            pp_spec(pat, arena, attr),
                            arena.alloc(Spec {
                                partial: false,
                                ..spec.clone()
                            }),
                        );
                });
            }
        }
    }
}

fn pp_fundef<'a>(arena: &'a Bump, attr: bool) -> impl Fn(&mut Doc<'a>, &'a Fundef) {
    move |doc, Fundef(id, ghost, kind, binders, pty_opt, pat, mask, spec, e)| {
        doc.print(pp_ghost, *ghost)
            .print(pp_kind, *kind)
            .print(pp_id(attr), id)
            .print(pp_binders(arena, attr), binders)
            .print(pp_opt_result(attr), (pty_opt, pat, mask))
            .print(pp_spec(pat, arena, attr), spec)
            .atom(" =")
            .space()
            .print(pp_expr(arena, attr).marked, e);
    }
}

/// Printer for expressions
pub fn pp_expr<'a>(arena: &'a Bump, attr: bool) -> Printers<'a, Expr> {
    let raw = move |doc: &mut Doc<'a>, e: &'a Expr| {
        match &e.desc {
            ExprDesc::Ref => {
                doc.atom("ref");
            }
            ExprDesc::True => pp_true(doc),
            ExprDesc::False => pp_false(doc),
            ExprDesc::Const(c) => pp_const(doc, arena, c),
            ExprDesc::Ident(qid) => pp_qualid(attr)(doc, qid),
            ExprDesc::Asref(qid) => pp_qualid(attr)(doc, qid),
            ExprDesc::Idapp(qid, es) => pp_idapp(&pp_expr(arena, attr), doc, qid, es, attr),
            ExprDesc::Apply(e1, e2) => {
                fn split_apply(e: &Expr) -> Option<(&Expr, &Expr)> {
                    match &e.desc {
                        ExprDesc::Apply(e1, e2) => Some((e1, e2)),
                        _ => None,
                    }
                }

                pp_apply(split_apply, &pp_expr(arena, attr), doc, e1, e2);
            }
            ExprDesc::Infix(e, op, e_) => {
                fn collect<'a>(mut op: &'a Ident, mut e: &'a Expr) -> Vec<(&'a Ident, &'a Expr)> {
                    let mut result: Vec<(_, &Expr)> = Vec::new();
                    loop {
                        if let ExprDesc::Infix(e__, op_, e_) = &e.desc {
                            result.push((op, e__));
                            op = op_;
                            e = e_;
                        } else {
                            result.push((op, e));
                            break result;
                        }
                    }
                }

                pp_infix(&pp_expr(arena, attr), doc, e, &collect(op, e_));
            }
            ExprDesc::Innfix(e1, op, e2) => pp_innfix(&pp_expr(arena, attr), doc, e1, op, e2),
            ExprDesc::Let(id, ghost, kind, e1, e2) => {
                match &e1.desc {
                    ExprDesc::Fun(binders, pty_opt, pat, mask, spec, e1_) => doc.vbox(0, |doc| {
                        doc.print(
                            pp_let_fun(&pp_expr(arena, attr), arena, attr),
                            (
                                e1.loc,
                                id,
                                *ghost,
                                *kind,
                                (binders, pty_opt, pat, mask, spec, e1_),
                            ),
                        )
                        .atom(" in")
                        .space()
                        .print(pp_expr(arena, attr).marked, e2);
                    }),
                    ExprDesc::Any(params, kind_, pty_opt, pat, mask, spec) => doc.vbox(0, |doc| {
                        doc.print(
                            pp_let_any(arena, attr),
                            (
                                e1.loc,
                                id,
                                *ghost,
                                *kind,
                                (params, *kind_, pty_opt, pat, mask, spec),
                            ),
                        )
                        .atom(" in")
                        .space()
                        .print(pp_expr(arena, attr).marked, e2);
                    }),
                    _ => doc.hvbox(0, |doc| {
                        doc.vbox(2, |doc| {
                            doc.print(
                                pp_let(&pp_expr(arena, attr), is_ref_expr, arena, attr),
                                (id, *ghost, *kind, e1),
                            );
                        })
                        .atom(" in")
                        .space()
                        .print(pp_expr(arena, attr).marked, e2);
                    }),
                };
            }
            ExprDesc::Rec(defs, e) => {
                doc.vbox(0, |doc| {
                    if defs.len() == 1 {
                        doc.vbox(2, |doc| {
                            doc.atom("let rec ")
                                .print(pp_fundef(arena, attr), &defs[0])
                                .atom(" in");
                        });
                    } else {
                        doc.vbox(2, |doc| {
                            doc.atom("let rec ").print(pp_fundef(arena, attr), &defs[0]);
                        })
                        .space()
                        .print_iter(
                            None::<fn(&mut _)>,
                            |doc, x| {
                                doc.hvbox(2, |doc| {
                                    doc.atom("with ").print(pp_fundef(arena, attr), x);
                                });
                            },
                            &defs[1..(defs.len() - 1)],
                        )
                        .hvbox(2, |doc| {
                            doc.atom("with ")
                                .print(pp_fundef(arena, attr), defs.last().unwrap())
                                .atom(" in");
                        });
                    }
                })
                .space()
                .print(pp_expr(arena, attr).marked, e);
            }
            ExprDesc::Fun(
                binders,
                None,
                pat @ Pattern {
                    desc: PatDesc::Wild,
                    ..
                },
                ity::Mask::Visible,
                spec,
                e,
            ) if binders.is_empty() => {
                let e = e.as_ref();
                if let Expr {
                    loc,
                    desc: ExprDesc::Tuple(x),
                } = e
                    && x.is_empty()
                {
                    doc.vbox(0, |doc| {
                        doc.vbox(2, |doc| {
                            doc.print(pp_maybe_marker, pat.loc)
                                .atom("begin")
                                .space()
                                .print(pp_maybe_marker, *loc)
                                .print(pp_spec(pat, arena, attr), spec);
                        })
                        .space()
                        .atom("end");
                    });
                } else {
                    doc.vbox(0, |doc| {
                        doc.vbox(2, |doc| {
                            doc.print(pp_maybe_marker, pat.loc)
                                .atom("begin")
                                .print(pp_spec(pat, arena, attr), spec)
                                .space()
                                .print(pp_expr(arena, attr).marked, e);
                        })
                        .space()
                        .atom("end");
                    });
                }
            }
            ExprDesc::Fun(binders, opt_pty, pat, mask, spec, e) => {
                pp_fun(
                    &pp_expr(arena, attr),
                    doc,
                    arena,
                    (binders, opt_pty, pat, mask, spec, e),
                    attr,
                );
            }
            ExprDesc::Any(params, _kind, Some(pty), pat, mask, spec) if params.is_empty() => {
                // TODO kind?
                let pat = if pat.desc != PatDesc::Wild {
                    pat
                } else if let [Post(_, x), ..] = &*spec.post
                    && let [(pat, _), ..] = x.as_ref()
                {
                    pat
                } else {
                    pat
                };
                let spec = spec.clone();
                let spec = arena.alloc(Spec {
                    pre: remove_witness_existence(spec.pre),
                    ..spec
                });
                doc.hvbox(2, |doc| {
                    doc.atom("any ")
                        .print(pp_pty_pat_mask(attr, true), (pty, (pat, mask)))
                        .print(pp_spec(pat, arena, attr), spec);
                });
            }
            ExprDesc::Any(..) => {
                // assert false?
                todo(doc, "EANY");
            }
            ExprDesc::Tuple(es) => pp_tuple(&pp_expr(arena, attr), doc, es),
            ExprDesc::Record(fs) => pp_record(&pp_expr(arena, attr), doc, fs, attr),
            ExprDesc::Update(e, fs) => pp_update(&pp_expr(arena, attr), doc, e, fs, attr),
            ExprDesc::Assign(x) if x.len() == 1 => {
                let (e1, oqid, e2) = &x[0];
                let pp_qid = |doc: &mut Doc<'a>, x| {
                    doc.atom(".").print(pp_qualid(attr), x);
                };
                doc.hvbox(2, |doc| {
                    doc.print(pp_expr(arena, attr).closed, e1)
                        .print(pp::print_option(pp_qid), oqid.as_ref())
                        .atom(" <-")
                        .space()
                        .print(pp_expr(arena, attr).closed, e2);
                });
            }
            ExprDesc::Assign(l) => {
                let pp_lhs = |doc: &mut Doc<'a>, (e, oqid, _): &'a _| {
                    if let Some(qid) = oqid {
                        doc.print(pp_expr(arena, attr).closed, e)
                            .atom(".")
                            .print(pp_qualid(attr), qid);
                    } else {
                        (pp_expr(arena, attr).closed)(doc, e);
                    }
                };
                let pp_rhs = |doc: &mut Doc<'a>, (_, _, e): &'a _| {
                    (pp_expr(arena, attr).closed)(doc, e);
                };
                doc.hvbox(2, |doc| {
                    doc.atom("(")
                        .print_iter(Some(pp::comma), pp_lhs, l)
                        .atom(") <-")
                        .space()
                        .atom("(")
                        .print_iter(Some(pp::comma), pp_rhs, l)
                        .atom(")");
                });
            }
            ExprDesc::Sequence(..) => {
                fn flatten(mut e: &Expr) -> Vec<&Expr> {
                    let mut result: Vec<&Expr> = Vec::new();
                    loop {
                        if let ExprDesc::Sequence(e1, e2) = &e.desc {
                            result.push(e1);
                            e = e2;
                        } else {
                            result.push(e);
                            break result;
                        }
                    }
                }

                doc.print_iter(
                    Some(|doc: &mut Doc| {
                        doc.atom(";").space();
                    }),
                    pp_expr(arena, attr).closed,
                    flatten(e),
                );
            }
            ExprDesc::If(e1, e2, e3) => pp_if(&pp_expr(arena, attr), doc, e1, e2, e3),
            ExprDesc::While(e1, invs, vars, e2) => {
                doc.vbox(0, |doc| {
                    doc.vbox(2, |doc| {
                        doc.atom("while ")
                            .print(pp_expr(arena, attr).marked, e1)
                            .atom(" do")
                            .print(pp_variants(arena, attr), vars)
                            .print(pp_invariants(arena, attr), invs)
                            .space()
                            .print(pp_expr(arena, attr).marked, e2);
                    })
                    .space()
                    .atom("done");
                });
            }
            ExprDesc::And(e1, e2) => {
                doc.sbox(0, |doc| {
                    doc.hvbox(2, |doc| {
                        doc.print(pp_expr(arena, attr).closed, e1);
                    })
                    .space()
                    .hvbox(2, |doc| {
                        doc.atom(" &&")
                            .space()
                            .print(pp_expr(arena, attr).closed, e2);
                    });
                });
            }
            ExprDesc::Or(e1, e2) => {
                doc.sbox(0, |doc| {
                    doc.hvbox(2, |doc| {
                        doc.print(pp_expr(arena, attr).closed, e1);
                    })
                    .space()
                    .hvbox(2, |doc| {
                        doc.atom(" ||")
                            .space()
                            .print(pp_expr(arena, attr).closed, e2);
                    });
                });
            }
            ExprDesc::Not(e) => pp_not(&pp_expr(arena, attr), doc, e),
            ExprDesc::Match(e, cases, xcases) if cases.is_empty() => {
                let pp_xcase = |doc: &mut Doc<'a>, (qid, opt_pat, e): &'a (_, Option<_>, _)| {
                    doc.print(pp_qualid(attr), qid)
                        .print(
                            pp_opt(
                                Doc::default(),
                                Doc::default(),
                                Doc::default(),
                                pp_pattern(attr).marked,
                            ),
                            opt_pat.as_ref(),
                        )
                        .atom(" ->")
                        .space()
                        .print(pp_expr(arena, attr).marked, e);
                };
                let pp_xcases = |doc: &mut Doc<'a>, x| {
                    doc.print_iter(
                        Some(|doc: &mut Doc| {
                            doc.space().atom("| ");
                        }),
                        pp_xcase,
                        x,
                    );
                };
                doc.hvbox(0, |doc| {
                    doc.hvbox(2, |doc| {
                        doc.atom("try")
                            .space()
                            .print(pp_expr(arena, attr).marked, e);
                    })
                    .space()
                    .hvbox(2, |doc| {
                        doc.atom("with").space().print(pp_xcases, xcases);
                    })
                    .space()
                    .atom("end");
                });
            }
            ExprDesc::Match(e, cases, xcases) => pp_match(
                &pp_expr(arena, attr),
                &pp_pattern(attr),
                doc,
                e,
                cases,
                xcases,
                attr,
            ),
            ExprDesc::Absurd => {
                doc.atom("absurd");
            }
            ExprDesc::Pure(t) => {
                doc.sbox(0, |doc| {
                    doc.hvbox(2, |doc| {
                        doc.atom("pure {")
                            .space()
                            .print(pp_term(arena, attr).marked, t)
                            .atom(" }");
                    })
                    .atom(" }");
                });
            }
            ExprDesc::Idpur(qid) => {
                doc.hbox(|doc| {
                    doc.atom("{ ").print(pp_qualid(attr), qid).atom(" }");
                });
            }
            ExprDesc::Raise(qid, opt_arg) => 'b: {
                'a: {
                    if let [id] = &*qid.0 {
                        let keyword = if &*id.str == ptree_helpers::RETURN_ID {
                            "return"
                        } else if &*id.str == ptree_helpers::BREAK_ID {
                            "break"
                        } else if &*id.str == ptree_helpers::CONTINUE_ID {
                            "continue"
                        } else {
                            break 'a;
                        };
                        doc.hvbox(2, |doc| {
                            doc.print(pp_maybe_marker, id.loc).atom(keyword).print(
                                pp_opt(
                                    {
                                        let mut doc: Doc = Doc::new();
                                        doc.atom(" ");
                                        doc
                                    },
                                    Doc::default(),
                                    Doc::default(),
                                    pp_expr(arena, attr).closed,
                                ),
                                opt_arg.as_deref(),
                            );
                        });
                    } else {
                        break 'a;
                    }
                    break 'b;
                }
                doc.atom("raise ").print(pp_qualid(attr), qid).print(
                    pp_opt(
                        {
                            let mut doc: Doc = Doc::new();
                            doc.atom(" ");
                            doc
                        },
                        Doc::default(),
                        Doc::default(),
                        pp_expr(arena, attr).closed,
                    ),
                    opt_arg.as_deref(),
                );
            }
            ExprDesc::Exn(id, pty, mask, e) => {
                doc.sbox(0, |doc| {
                    doc.print(pp_exn(attr), (id, pty, mask))
                        .atom(" in")
                        .space()
                        .print(pp_expr(arena, attr).marked, e);
                });
            }
            ExprDesc::Optexn(id, _mask, e)
                if {
                    use ptree_helpers::*;
                    [RETURN_ID, BREAK_ID, CONTINUE_ID]
                }
                .iter()
                .any(|s| id.str.ends_with(s))
                    && marker(id.loc).is_none() =>
            {
                // Syntactic sugar
                (pp_expr(arena, attr).marked)(doc, e);
            }
            ExprDesc::Optexn(id, mask, e) => {
                if mask != &ity::Mask::Visible {
                    // no possible concrete syntax
                    todo(doc, "OPTEXN mask<>visible");
                } else {
                    doc.vbox(0, |doc| {
                        doc.atom("exception ")
                            .print(pp_id(attr), id)
                            .atom(" in")
                            .space()
                            .print(pp_expr(arena, attr).marked, e);
                    });
                }
            }
            ExprDesc::For(id, start, dir, end, invs, body) => {
                let dir = match dir {
                    expr::ForDirection::To => "to",
                    expr::ForDirection::DownTo => "downto",
                };
                doc.vbox(0, move |doc| {
                    doc.vbox(2, move |doc| {
                        doc.atom("for ")
                            .print(pp_id(attr), id)
                            .atom(" = ")
                            .print(pp_expr(arena, attr).marked, start)
                            .atom_fn(move |f| write!(f, " {dir} "))
                            .print(pp_expr(arena, attr).marked, end)
                            .atom(" do")
                            .print(pp_invariants(arena, attr), invs)
                            .space()
                            .print(pp_expr(arena, attr).marked, body);
                    })
                    .space()
                    .atom("done");
                });
            }
            ExprDesc::Assert(
                expr::AssertionKind::Assert,
                Term {
                    loc,
                    desc: TermDesc::Attr(Attr::Str(ident::Attribute(string)), t),
                },
            ) if string == "hyp_name:Assert" => {
                doc.hvbox(2, |doc| {
                    doc.atom("assert {")
                        .space()
                        .print(pp_maybe_marker, *loc)
                        .print(pp_term(arena, attr).marked, t)
                        .atom(" }");
                });
            }
            ExprDesc::Assert(
                expr::AssertionKind::Assume,
                Term {
                    loc,
                    desc: TermDesc::Attr(Attr::Str(ident::Attribute(string)), t),
                },
            ) if string == "hyp_name:Assume" => {
                doc.hvbox(2, |doc| {
                    doc.atom("assume {")
                        .space()
                        .print(pp_maybe_marker, *loc)
                        .print(pp_term(arena, attr).marked, t)
                        .atom(" }");
                });
            }
            ExprDesc::Assert(
                expr::AssertionKind::Check,
                Term {
                    loc,
                    desc: TermDesc::Attr(Attr::Str(ident::Attribute(string)), t),
                },
            ) if string == "hyp_name:Check" => {
                doc.hvbox(2, |doc| {
                    doc.atom("check {")
                        .space()
                        .print(pp_maybe_marker, *loc)
                        .print(pp_term(arena, attr).marked, t)
                        .atom(" }");
                });
            }
            ExprDesc::Assert(kind, t) => {
                let kind = match kind {
                    expr::AssertionKind::Assert => "assert",
                    expr::AssertionKind::Assume => "assume",
                    expr::AssertionKind::Check => "check",
                };
                let (oloc, s, t) = term_hyp_name(t);
                doc.hvbox(2, move |doc| {
                    doc.atom_fn(move |f| write!(f, "{kind}{s} {{"))
                        .space()
                        .print(pp::print_option(pp_maybe_marker), oloc)
                        .print(pp_term(arena, attr).marked, t)
                        .atom(" }");
                });
            }
            ExprDesc::Scope(qid, e) => pp_scope(&pp_expr(arena, attr), doc, qid, e, attr),
            ExprDesc::Label(id, e) => {
                doc.hvbox(2, |doc| {
                    doc.atom("label ")
                        .print(pp_id(attr), id)
                        .atom(" in")
                        .space()
                        .print(pp_expr(arena, attr).marked, e);
                });
            }
            ExprDesc::Cast(e, pty) => pp_cast(&pp_expr(arena, attr), doc, e, pty, attr),
            ExprDesc::Ghost(e) => {
                doc.atom("ghost ").print(pp_expr(arena, attr).closed, e);
            }
            ExprDesc::Attr(Attr::Str(att), e) if att == &*ident::FUNLIT => {
                pp_e_funlit(doc, arena, e, attr)
            }
            ExprDesc::Attr(att, e) => {
                fn expr_closed(e: &Expr) -> bool {
                    if matches!(
                        e,
                        Expr {
                            desc: ExprDesc::Attr(..),
                            ..
                        }
                    ) {
                        true
                    } else {
                        expr_closed(e)
                    }
                }

                pp_attr(
                    pp_closed(expr_closed, pp_expr(arena, attr).marked),
                    doc,
                    att,
                    e,
                );
            }
        }
    };
    let marked = move |doc: &mut _, e| pp_maybe_marked(None, |e| e.loc, raw, doc, e);
    let closed = pp_closed(expr_closed, marked);
    Printers {
        marked: Box::new(marked),
        closed: Box::new(closed),
    }
}

/// Printer for terms
pub fn pp_term<'a>(arena: &'a Bump, attr: bool) -> Printers<'a, Term> {
    let raw = move |doc: &mut Doc<'a>, t: &'a Term| {
        fn pp_binop(doc: &mut Doc, op: dterm::Dbinop) {
            let op_str = match op {
                dterm::Dbinop::And => r"/\",
                dterm::Dbinop::AndAsym => "&&",
                dterm::Dbinop::Or => r"\/",
                dterm::Dbinop::OrAsym => "||",
                dterm::Dbinop::Implies => "->",
                dterm::Dbinop::Iff => "<->",
                dterm::Dbinop::By => "by",
                dterm::Dbinop::So => "so",
            };
            doc.atom(op_str);
        }

        match &t.desc {
            TermDesc::True => pp_true(doc),
            TermDesc::False => pp_false(doc),
            TermDesc::Const(c) => pp_const(doc, arena, c),
            TermDesc::Ident(qid) => pp_qualid(attr)(doc, qid),
            TermDesc::Asref(qid) => pp_qualid(attr)(doc, qid),
            TermDesc::Idapp(qid, ts) => pp_idapp(&pp_term(arena, attr), doc, qid, ts, attr),
            TermDesc::Apply(t1, t2) => {
                fn split_apply(t: &Term) -> Option<(&Term, &Term)> {
                    match &t.desc {
                        TermDesc::Apply(t1, t2) => Some((t1, t2)),
                        _ => None,
                    }
                }

                pp_apply(split_apply, &pp_term(arena, attr), doc, t1, t2);
            }
            TermDesc::Infix(t, op, t_) => {
                fn collect<'a>(mut op: &'a Ident, mut t: &'a Term) -> Vec<(&'a Ident, &'a Term)> {
                    let mut result: Vec<(_, &Term)> = Vec::new();
                    loop {
                        if let TermDesc::Infix(t__, op_, t_) = &t.desc {
                            result.push((op, t__));
                            op = op_;
                            t = t_;
                        } else {
                            result.push((op, t));
                            break result;
                        }
                    }
                }

                pp_infix(&pp_term(arena, attr), doc, t, &collect(op, t_));
            }
            TermDesc::Innfix(t1, op, t2) => pp_innfix(&pp_term(arena, attr), doc, t1, op, t2),
            TermDesc::Binop(t, op, t_) => {
                fn collect(mut op: dterm::Dbinop, mut t: &Term) -> Vec<(dterm::Dbinop, &Term)> {
                    let mut result: Vec<(dterm::Dbinop, &Term)> = Vec::new();
                    loop {
                        if let TermDesc::Binop(t__, op_, t_) = &t.desc {
                            result.push((op, t__));
                            op = *op_;
                            t = t_;
                        } else {
                            result.push((op, t));
                            break result;
                        }
                    }
                }

                let pp_op = enforce_same_lifetime(|doc, (op, t): &(_, &_)| {
                    doc.print(pp_binop, *op)
                        .space()
                        .print(pp_term(arena, attr).closed, t);
                });
                doc.hvbox(2, |doc| {
                    doc.print(pp_term(arena, attr).closed, t).space().print(
                        pp_print_opt_list(
                            Doc::default(),
                            &Doc::default(),
                            &{
                                let mut doc = Doc::new();
                                doc.space();
                                doc
                            },
                            Doc::default(),
                            Doc::default(),
                            pp_op,
                        ),
                        arena.alloc(collect(*op, t_)) as &_,
                    );
                });
            }
            TermDesc::Binnop(t1, op, t2) => {
                doc.hvbox(3, |doc| {
                    doc.atom("(")
                        .print(pp_term(arena, attr).closed, t1)
                        .atom(" ")
                        .print(pp_binop, *op)
                        .space()
                        .print(pp_term(arena, attr).closed, t2)
                        .atom(")");
                });
            }
            TermDesc::Not(t) => pp_not(&pp_term(arena, attr), doc, t),
            TermDesc::If(t1, t2, t3) => pp_if(&pp_term(arena, attr), doc, t1, t2, t3),
            TermDesc::Quant(quant, binders, triggers, t) => {
                let (quant, sep, pp_binders): (
                    _,
                    _,
                    Box<dyn Fn(bool) -> Box<dyn FnOnce(&mut Doc<'a>, &'a [Binder])>>,
                ) = match quant {
                    dterm::Dquant::Forall => (
                        "forall",
                        ".",
                        Box::new(|attr| Box::new(pp_comma_binders(attr))),
                    ),
                    dterm::Dquant::Exists => (
                        "exists",
                        ".",
                        Box::new(|attr| Box::new(pp_comma_binders(attr))),
                    ),
                    dterm::Dquant::Lambda => (
                        "fun",
                        "->",
                        Box::new(move |attr| Box::new(pp_binders(arena, attr))),
                    ),
                };
                let pp_terms = |doc: &mut Doc<'a>, x| {
                    doc.print_iter(
                        Some(|doc: &mut Doc| {
                            doc.atom(", ");
                        }),
                        pp_term(arena, attr).marked,
                        x,
                    );
                };
                let pp_triggers = pp_print_opt_list(
                    {
                        let mut doc: Doc = Doc::new();
                        doc.space().atom("[");
                        doc
                    },
                    Doc::default(),
                    {
                        let mut doc: Doc = Doc::new();
                        doc.space().atom(" | ");
                        doc
                    },
                    {
                        let mut doc: Doc = Doc::new();
                        doc.space().atom("]");
                        doc
                    },
                    Doc::default(),
                    pp_terms,
                );
                doc.hvbox(2, |doc| {
                    doc.atom(quant)
                        .hvbox(0, |doc| {
                            doc.print(pp_binders(attr), binders)
                                .print(pp_triggers, triggers);
                        })
                        .atom(sep)
                        .space()
                        .print(pp_term(arena, attr).marked, t);
                });
            }
            TermDesc::Eps(id, ty, f) => {
                doc.hvbox(2, |doc| {
                    doc.atom("epsilon ")
                        .print(pp_id(attr), id)
                        .atom(":")
                        .print(pp_pty(attr).marked, ty)
                        .atom(".")
                        .space()
                        .print(pp_term(arena, attr).marked, f);
                });
            }
            TermDesc::Attr(Attr::Str(att), t) if att == &*ident::FUNLIT => {
                pp_t_funlit(doc, arena, t, attr)
            }
            TermDesc::Attr(att, t) => {
                fn term_closed(t: &Term) -> bool {
                    if matches!(t.desc, TermDesc::Attr(..)) {
                        true
                    } else {
                        term_closed(t)
                    }
                }

                pp_attr(
                    pp_closed(term_closed, pp_term(arena, attr).marked),
                    doc,
                    att,
                    t,
                );
            }
            TermDesc::Let(id, t1, t2) => {
                doc.vbox(0, |doc| {
                    doc.print(
                        pp_let(&pp_term(arena, attr), is_ref_pattern, arena, attr),
                        (id, false, expr::RsKind::None, t1),
                    )
                    .atom(" in")
                    .space()
                    .print(pp_term(arena, attr).marked, t2);
                });
            }
            TermDesc::Case(t, cases) => pp_match(
                &pp_term(arena, attr),
                &pp_pattern(attr),
                doc,
                t,
                cases,
                &[],
                attr,
            ),
            TermDesc::Cast(t, pty) => pp_cast(&pp_term(arena, attr), doc, t, pty, attr),
            TermDesc::Tuple(ts) => pp_tuple(&pp_term(arena, attr), doc, ts),
            TermDesc::Record(fs) => pp_record(&pp_term(arena, attr), doc, fs, attr),
            TermDesc::Update(t, fs) => pp_update(&pp_term(arena, attr), doc, t, fs, attr),
            TermDesc::Scope(qid, t) => pp_scope(&pp_term(arena, attr), doc, qid, t, attr),
            TermDesc::At(t, Ident { str, loc, .. }) if str.as_ref() == "Old" => {
                doc.print(pp_maybe_marker, *loc)
                    .atom("old ")
                    .print(pp_term(arena, attr).closed, t);
            }
            TermDesc::At(t, id) => {
                doc.print(pp_term(arena, attr).closed, t)
                    .atom(" at ")
                    .print(pp_id(attr), id);
            }
        }
    };
    let marked = move |doc: &mut Doc<'a>, t| pp_maybe_marked(None, |t| t.loc, raw, doc, t);
    let closed = pp_closed(term_closed, marked);
    Printers {
        closed: Box::new(closed),
        marked: Box::new(marked),
    }
}

fn pp_t_funlit<'a>(doc: &mut Doc<'a>, arena: &'a Bump, t: &'a Term, attr: bool) {
    fn enforce_lifetime<'a, T: Fn(&mut Doc<'a>, &'a Term), U: Fn(&'a Ident) -> T>(closure: U) -> U {
        closure
    }

    let print_elems = enforce_lifetime(|var| {
        move |doc, mut t| {
            loop {
                if let TermDesc::If(x, t2, t3) = &t.desc
                    && let Term {
                        desc: TermDesc::Infix(x, _, t_),
                        ..
                    } = x.as_ref()
                    && let Term {
                        desc: TermDesc::Ident(Qualid(x)),
                        ..
                    } = x.as_ref()
                    && let [v] = x.as_ref()
                    && var == v
                {
                    doc.print(pp_term(arena, attr).marked, t_)
                        .atom(" => ")
                        .print(pp_term(arena, attr).marked, t2)
                        .atom(";");
                    t = t3;
                } else if let TermDesc::Idapp(Qualid(x), _) = &t.desc
                    && let [Ident { str, .. }] = x.as_ref()
                    && str.as_ref() == "any function"
                {
                    break;
                } else {
                    doc.atom("_ => ").print(pp_term(arena, attr).marked, t);
                    break;
                }
            }
        }
    });
    if let TermDesc::Quant(dterm::Dquant::Lambda, x, _, t) = &t.desc
        && let [Binder(_, Some(var), _, _)] = x.as_ref()
    {
        doc.atom("[|").print(print_elems(var), t).atom("|]");
    } else {
        // should not happen
        panic!();
    }
}

fn pp_e_funlit<'a>(doc: &mut Doc<'a>, arena: &'a Bump, e: &'a Expr, attr: bool) {
    fn enforce_lifetime_0<'a, T: Fn(&Ident, &Expr, &mut Doc<'a>, &[(&Ident, &'a Expr)])>(
        closure: T,
    ) -> T {
        closure
    }

    fn enforce_lifetime_1<
        'a,
        T: FnMut(&mut Doc<'a>, &'a Expr),
        U: Fn(Vec<(&'a Ident, &'a Expr)>) -> T,
    >(
        closure: U,
    ) -> U {
        closure
    }

    let substitute_and_print = enforce_lifetime_0(|var, mut e, doc, mut l| {
        loop {
            if let ExprDesc::If(e1, e2, e3) = &e.desc
                && let Expr {
                    desc: ExprDesc::Infix(v, _, id1),
                    ..
                } = e1.as_ref()
                && let Expr {
                    desc: ExprDesc::Ident(Qualid(v)),
                    ..
                } = v.as_ref()
                && let [v] = v.as_ref()
                && let Expr {
                    desc: ExprDesc::Ident(Qualid(id1)),
                    ..
                } = id1.as_ref()
                && let [id1] = id1.as_ref()
                && let Expr {
                    desc: ExprDesc::Ident(Qualid(id2)),
                    ..
                } = e2.as_ref()
                && let [id2] = id2.as_ref()
                && let [tl @ .., (id4, e5), (id3, e4)] = l
                && var == v
                && id1 == *id4
                && id2 == *id3
            {
                doc.print(pp_expr(arena, attr).marked, e5)
                    .atom(" => ")
                    .print(pp_expr(arena, attr).marked, e4)
                    .atom(";");
                e = e3;
                l = tl;
            } else if let ExprDesc::Ident(Qualid(id1)) = &e.desc
                && let [id1] = id1.as_ref()
                && let [(id2, e)] = l
                && id1 == *id2
            {
                doc.atom("_ => ").print(pp_expr(arena, attr).marked, e);
                break;
            } else if let ExprDesc::Ident(Qualid(x)) = &e.desc
                && let [_] = x.as_ref()
                && l.is_empty()
            {
                break;
            } else {
                // should not happen
                panic!();
            }
        }
    });
    let unfold_let = enforce_lifetime_1(|mut acc| {
        move |doc, mut e| loop {
            if let ExprDesc::Let(id, false, expr::RsKind::None, e1, e2) = &e.desc {
                acc.push((id, e1));
                e = e2;
            } else if let ExprDesc::Fun(x, _, _, _, _, e) = &e.desc
                && let [Binder(_, Some(var), _, _)] = x.as_ref()
            {
                substitute_and_print(var, e, doc, &acc);
                break;
            } else {
                // should not happen
                panic!();
            }
        }
    });
    doc.atom("[|").print(unfold_let(Vec::new()), e).atom("|]");
}

fn pp_variants<'a>(arena: &'a Bump, attr: bool) -> impl Fn(&mut Doc<'a>, &'a Variant) {
    move |doc, vs| {
        let pp = match vs {
            [] | [_] => (pp_term(arena, attr)).marked,
            _ => (pp_term(arena, attr)).closed,
        };
        let pp_variant = |doc: &mut Doc<'a>, (t, qid_opt): &'a (_, Option<_>)| {
            doc.print(&pp, t).print(
                pp_opt(
                    {
                        let mut doc: Doc = Doc::new();
                        doc.atom(" with ");
                        doc
                    },
                    Doc::default(),
                    Doc::default(),
                    pp_qualid(attr),
                ),
                qid_opt.as_ref(),
            );
        };
        if !vs.is_empty() {
            doc.space()
                .hvbox(2, |doc| {
                    doc.atom("variant {").space().print_iter(
                        Some(|doc: &mut Doc| {
                            doc.atom(",").space();
                        }),
                        pp_variant,
                        vs,
                    );
                })
                .atom(" }");
        }
    }
}

fn pp_invariants<'a>(arena: &'a Bump, attr: bool) -> impl FnOnce(&mut Doc<'a>, &'a Invariant) {
    let pp_invariant = move |doc: &mut Doc<'a>, t: &'a _| {
        let (oloc, s, t) = term_hyp_name(t);
        let t = remove_term_attr(
            "hyp_name:LoopInvariant",
            remove_term_attr("hyp_name:TypeInvariant", t),
        );
        doc.hvbox(2, move |doc| {
            doc.atom_fn(move |f| write!(f, "invariants{s} {{"))
                .space()
                .print(pp::print_option(pp_maybe_marker), oloc)
                .print(pp_term(arena, attr).marked, t);
        })
        .atom(" }");
    };
    pp_print_opt_list(
        {
            let mut doc: Doc = Doc::new();
            doc.space();
            doc
        },
        Doc::default(),
        {
            let mut doc: Doc = Doc::new();
            doc.space();
            doc
        },
        Doc::default(),
        Doc::default(),
        pp_invariant,
    )
}

// result_pat is not printed, only used to print the post-conditions
fn pp_spec<'a>(
    result_pat: &Pattern,
    arena: &'a Bump,
    attr: bool,
) -> impl Fn(&mut Doc<'a>, &'a Spec) {
    move |doc, s| {
        let pp_requires = |doc: &mut Doc<'a>, t| {
            let t = remove_term_attr("hyp_name:Requires", t);
            let (oloc, s, t) = term_hyp_name(t);
            doc.hvbox(0, move |doc| {
                doc.hvbox(2, move |doc| {
                    doc.atom_fn(move |f| write!(f, "requires{s} {{ "))
                        .print(pp::print_option(pp_maybe_marker), oloc)
                        .print(pp_term(arena, attr).marked, t);
                })
                .space()
                .atom("}");
            });
        };
        // let f x : (p: ty) returns { p -> t }
        let is_ensures = |pat: &Pattern| match (&result_pat.desc, &pat.desc) {
            (PatDesc::Wild, PatDesc::Var(id)) => &*id.str == "result",
            _ => pat_equals(result_pat, pat),
        };

        fn is_marked_id(id: &Ident) -> bool {
            marker(id.loc).is_some()
        }

        fn is_marked_qid(qid: &Qualid) -> bool {
            qid.0.iter().any(is_marked_id)
        }

        fn is_marked(pat: &Pattern) -> bool {
            if marker(pat.loc).is_some() {
                return true;
            }
            match &pat.desc {
                PatDesc::Wild => false,
                PatDesc::Var(id) => is_marked_id(id),
                PatDesc::App(qid, ps) => is_marked_qid(qid) || ps.iter().any(is_marked),
                PatDesc::Rec(fs) => fs.iter().any(|(qid, p)| is_marked_qid(qid) || is_marked(p)),
                PatDesc::Tuple(ps) => ps.iter().any(is_marked),
                PatDesc::As(p1, id, _) => is_marked(p1) || is_marked_id(id),
                PatDesc::Or(p1, p2) => is_marked(p1) || is_marked(p2),
                PatDesc::Cast(p, _) => is_marked(p),
                PatDesc::Scope(qid, p) => is_marked_qid(qid) || is_marked(p),
                PatDesc::Paren(p) => is_marked(p),
                PatDesc::Ghost(p) => is_marked(p),
            }
        }

        let pp_post = |doc: &mut Doc<'a>, Post(loc, cases): &'a _| {
            if let [(pat, t)] = cases.as_ref()
                && is_ensures(pat)
                && !(is_marked(pat))
            {
                let t = remove_term_attr("hyp_name:Ensures", t);
                let (oloc, s, t) = term_hyp_name(t);
                doc.space()
                    .hvbox(2, |doc| {
                        doc.print(pp_maybe_marker, *loc)
                            .atom_fn(move |f| write!(f, "ensures{s} {{ "))
                            .print(pp::print_option(pp_maybe_marker), oloc)
                            .print(pp_term(arena, attr).marked, t);
                    })
                    .atom(" }");
            } else {
                let pp_case = |doc: &mut Doc<'a>, (p, t): &'a _| {
                    doc.print(pp_pattern(attr).marked, p)
                        .atom(" -> ")
                        .print(pp_term(arena, attr).marked, t);
                };
                doc.space()
                    .print(pp_maybe_marker, *loc)
                    .atom("returns { ")
                    .space()
                    .print_iter(
                        Some(|doc: &mut Doc| {
                            doc.atom("|");
                        }),
                        pp_case,
                        cases,
                    )
                    .atom(" }");
            }
        };
        let pp_xpost = |doc: &mut Doc<'a>, Xpost(loc, exn_cases): &'a _| {
            let pp_exn_case = |doc: &mut Doc<'a>, (qid, opt_pat_term): &'a _| {
                let pp_opt_t = |doc: &mut Doc<'a>, x: &'a _| {
                    if let Some((p, t)) = x {
                        doc.atom(" ")
                            .print(pp_pattern(attr).marked, p)
                            .atom(" -> ")
                            .print(pp_term(arena, attr).marked, t);
                    }
                };
                doc.hvbox(2, |doc| {
                    doc.print(pp_qualid(attr), qid)
                        .print(pp_opt_t, opt_pat_term);
                });
            };
            doc.hvbox(2, |doc| {
                doc.print(pp_maybe_marker, *loc)
                    .atom("raises { ")
                    .print_iter(
                        Some(|doc: &mut Doc| {
                            doc.space().atom("| ");
                        }),
                        pp_exn_case,
                        exn_cases,
                    )
                    .atom(" }");
            });
        };
        let pp_alias = |doc: &mut Doc<'a>, (t1, t2): &'a _| {
            doc.print(pp_term(arena, attr).marked, t1)
                .atom(" with ")
                .print(pp_term(arena, attr).marked, t2);
        };
        if !s.reads.is_empty() {
            doc.space()
                .hvbox(2, |doc| {
                    doc.atom("reads { ").print_iter(
                        Some(|doc: &mut Doc| {
                            doc.atom(",").space();
                        }),
                        pp_qualid(attr),
                        &s.reads,
                    );
                })
                .atom(" }");
        }
        pp_print_opt_list(
            {
                let mut doc: Doc = Doc::new();
                doc.space();
                doc
            },
            &Doc::default(),
            &{
                let mut doc: Doc = Doc::new();
                doc.space();
                doc
            },
            Doc::default(),
            Doc::default(),
            pp_requires,
        )(doc, &s.pre);
        if s.checkrw {
            doc.space().hbox(|doc| {
                doc.atom("writes { ");
                pp_print_opt_list(
                    Doc::default(),
                    &Doc::default(),
                    &{
                        let mut doc: Doc = Doc::new();
                        doc.atom(",").space();
                        doc
                    },
                    Doc::default(),
                    Doc::default(),
                    pp_term(arena, attr).marked,
                )(doc, &s.writes);
                doc.atom(" }");
            });
        } else {
            if !s.writes.is_empty() {
                doc.space()
                    .hvbox(2, |doc| {
                        doc.atom("writes { ").print_iter(
                            Some(|doc: &mut Doc| {
                                doc.atom(",").space();
                            }),
                            pp_term(arena, attr).marked,
                            &s.writes,
                        );
                    })
                    .atom(" }");
            }
        }
        doc.print_iter(None::<fn(&mut _)>, pp_post, &s.post);
        pp_print_opt_list(
            {
                let mut doc: Doc = Doc::new();
                doc.space();
                doc
            },
            &Doc::default(),
            &{
                let mut doc: Doc = Doc::new();
                doc.space();
                doc
            },
            Doc::default(),
            Doc::default(),
            pp_xpost,
        )(doc, &s.xpost);
        if !s.alias.is_empty() {
            doc.hvbox(2, |doc| {
                doc.atom("alias { ").print_iter(
                    Some(|doc: &mut Doc| {
                        doc.atom(",").space();
                    }),
                    pp_alias,
                    &s.alias,
                );
            })
            .atom(" }");
        }
        pp_variants(arena, attr)(doc, &s.variant);
        pp_bool(
            Some({
                let mut doc: Doc = Doc::new();
                doc.space().atom("diverges");
                doc
            }),
            None,
            doc,
            s.diverge,
        );
        pp_bool(
            Some({
                let mut doc: Doc = Doc::new();
                doc.space().atom("partial");
                doc
            }),
            None,
            doc,
            s.partial,
        );
    }
}

/// Printer for patterns
pub fn pp_pattern<'a>(attr: bool) -> Printers<'a, Pattern> {
    let pp_pattern_raw = enforce_same_lifetime(move |doc, p: &Pattern| match &p.desc {
        PatDesc::Wild => {
            doc.atom("_");
        }
        PatDesc::Var(id) => pp_id(attr)(doc, id),
        PatDesc::App(qid, args) => pp_idapp(&pp_pattern(attr), doc, qid, args, attr),
        PatDesc::Rec(fields) => {
            let pp_field = enforce_same_lifetime(|doc, (qid, pat): &_| {
                doc.print(pp_qualid(attr), qid)
                    .atom(" = ")
                    .print(pp_pattern(attr).closed, pat);
            });
            doc.atom("{ ")
                .print_iter(
                    Some(|doc: &mut Doc| {
                        doc.atom(";").space();
                    }),
                    pp_field,
                    fields,
                )
                .atom(" }");
        }
        PatDesc::Tuple(ps) => pp_tuple(&pp_pattern(attr), doc, ps),
        PatDesc::As(p, id, ghost) => {
            doc.hvbox(2, |doc| {
                doc.print(pp_pattern(attr).marked, p);
            })
            .atom(" as")
            .space()
            .print(pp_ghost, *ghost)
            .print(pp_id(attr), id);
        }
        PatDesc::Or(p1, p2) => {
            doc.print(pp_pattern(attr).marked, p1)
                .atom(" | ")
                .print(pp_pattern(attr).marked, p2);
        }
        PatDesc::Cast(p, pty) => pp_cast(&pp_pattern(attr), doc, p, pty, attr),
        PatDesc::Scope(qid, p) => pp_scope(&pp_pattern(attr), doc, qid, p, attr),
        PatDesc::Paren(p) => {
            doc.atom("(").print(pp_pattern(attr).marked, p).atom(")");
        }
        PatDesc::Ghost(p) => {
            doc.hbox(|doc| {
                doc.atom("ghost").space().print(pp_pattern(attr).marked, p);
            });
        }
    });
    let marked = enforce_same_lifetime(move |doc, p| {
        pp_maybe_marked(None, |p| p.loc, pp_pattern_raw, doc, p)
    });
    let closed = pp_closed(pattern_closed, marked);
    Printers {
        marked: Box::new(marked),
        closed: Box::new(closed),
    }
}

fn pp_type_decl<'a>(arena: &'a Bump, attr: bool) -> impl Fn(&mut Doc<'a>, &'a TypeDecl) {
    move |doc, d| {
        let pp_def = |doc: &mut Doc<'a>, x: &'a _| match x {
            TypeDef::Alias(pty) => {
                doc.atom(" = ").print(pp_pty(attr).marked, pty);
            }
            TypeDef::Record(fs) if fs.is_empty() && d.vis == Visibility::Abstract && !d.r#mut => (),
            TypeDef::Record(fs) => {
                let vis = match d.vis {
                    Visibility::Public => "",
                    Visibility::Private => "private ",
                    Visibility::Abstract => "abstract ",
                };
                let pp_field = enforce_same_lifetime(|doc, f: &Field| {
                    doc.hvbox(2, |doc| {
                        doc.print(pp_maybe_marker, f.loc)
                            .print(pp_mutable, f.mutable)
                            .print(pp_ghost, f.ghost)
                            .print(pp_id(attr), &f.ident)
                            .atom(" :")
                            .space()
                            .print(pp_pty(attr).marked, &f.pty);
                    });
                });
                let pp_fields = enforce_same_lifetime(|doc, x: &[_]| {
                    doc.print_iter(
                        Some(|doc: &mut Doc| {
                            doc.atom(";").space();
                        }),
                        pp_field,
                        x,
                    );
                });
                doc.hvbox(2, move |doc| {
                    doc.atom_fn(move |f| write!(f, " = {vis}"))
                        .print(pp_mutable, d.r#mut)
                        .atom("{")
                        .space()
                        .print(pp_fields, fs)
                        .space()
                        .atom("}");
                })
                .print(pp_invariants(arena, attr), &d.inv);
            }
            TypeDef::Algebraic(variants) => {
                let pp_variant = |doc: &mut Doc<'a>, (loc, id, params): &'a (_, _, Box<_>)| {
                    doc.print(pp_maybe_marker, *loc)
                        .print(pp_id(attr), id)
                        .print(pp_params(arena, attr), params);
                };
                doc.atom(" = ").cut().vbox(2, |doc| {
                    doc.atom("  | ").print_iter(
                        Some(|doc: &mut Doc| {
                            doc.cut().atom("| ");
                        }),
                        pp_variant,
                        variants,
                    );
                });
            }
            TypeDef::Range(i1, i2) => {
                doc.atom_fn(move |f| write!(f, " = <range {i1} {i2}>"));
            }
            TypeDef::Float(i1, i2) => {
                doc.atom_fn(|f| write!(f, " = <float {} {}>", *i1, *i2));
            }
        };
        doc.print(pp_maybe_marker, d.loc)
            .print(pp_id(attr), &d.ident)
            .print(
                pp_print_opt_list(
                    Doc::default(),
                    &{
                        let mut doc: Doc = Doc::new();
                        doc.atom(" '");
                        doc
                    },
                    &Doc::default(),
                    Doc::default(),
                    Doc::default(),
                    pp_id(attr),
                ),
                &d.params,
            )
            .print(pp_def, &d.def);
        if let Some(x) = &d.wit {
            doc.space().hvbox(2, |doc| {
                doc.atom("by").space().print(pp_expr(arena, attr).closed, x);
            });
        }
    }
}

fn pp_ind_decl<'a>(arena: &'a Bump, attr: bool) -> impl Fn(&mut Doc<'a>, &'a IndDecl) {
    move |doc, d| {
        let pp_ind_decl_case = |doc: &mut Doc<'a>, (loc, id, t): &'a _| {
            doc.print(pp_maybe_marker, *loc)
                .print(pp_id(attr), id)
                .atom(" : ")
                .print(pp_term(arena, attr).marked, t);
        };
        let pp_ind_decl_def = |doc: &mut Doc<'a>, x: &'a [_]| {
            doc.print_iter(
                Some(|doc: &mut Doc| {
                    doc.atom(" | ");
                }),
                pp_ind_decl_case,
                x,
            );
        };
        doc.print(pp_maybe_marker, d.loc)
            .print(pp_id(attr), &d.ident)
            .print(
                pp_print_opt_list(
                    {
                        let mut doc: Doc = Doc::new();
                        doc.atom(" ");
                        doc
                    },
                    &Doc::default(),
                    &Doc::default(),
                    Doc::default(),
                    Doc::default(),
                    pp_param(arena, attr),
                ),
                &d.params,
            )
            .atom(" = ")
            .print(pp_ind_decl_def, &d.def);
    }
}

fn pp_logic_decl<'a>(arena: &'a Bump, attr: bool) -> impl Fn(&mut Doc<'a>, &'a LogicDecl) {
    move |doc, d| {
        doc.print(pp_maybe_marker, d.loc)
            .print(pp_id(attr), &d.ident)
            .print(
                pp_print_opt_list(
                    {
                        let mut doc: Doc = Doc::new();
                        doc.atom(" ");
                        doc
                    },
                    &Doc::default(),
                    &{
                        let mut doc: Doc = Doc::new();
                        doc.atom(" ");
                        doc
                    },
                    Doc::default(),
                    Doc::default(),
                    pp_param(arena, attr),
                ),
                &d.params,
            )
            .print(
                pp_opt(
                    {
                        let mut doc: Doc = Doc::new();
                        doc.atom(" : ");
                        doc
                    },
                    Doc::default(),
                    Doc::default(),
                    pp_pty(attr).marked,
                ),
                d.r#type.as_ref(),
            )
            .print(
                pp_opt(
                    {
                        let mut doc: Doc = Doc::new();
                        doc.atom(" =").space();
                        doc
                    },
                    Doc::default(),
                    Doc::default(),
                    pp_term(arena, attr).marked,
                ),
                d.def.as_ref(),
            );
    }
}

/// Printer for declarations
pub fn pp_decl<'a>(attr: Option<bool>, arena: &'a Bump) -> impl Fn(&mut Doc<'a>, &'a Decl) {
    let attr = attr.unwrap_or(true);

    move |doc, x| {
        match x {
            Decl::Type(decls) => {
                doc.vbox(2, |doc| {
                    doc.atom("type ")
                        .print(pp_type_decl(arena, attr), &decls[0]);
                });
                for decl in &decls[1..] {
                    doc.space().vbox(2, |doc| {
                        doc.atom("with ").print(pp_type_decl(arena, attr), decl);
                    });
                }
            }
            Decl::Logic(decls) if decls.iter().all(|d| d.r#type.is_none()) => {
                // predicates don't have an type
                doc.hvbox(2, |doc| {
                    doc.atom("predicate ")
                        .print(pp_logic_decl(arena, attr), &decls[0]);
                });
                for decl in &decls[1..] {
                    doc.space().hvbox(2, |doc| {
                        doc.atom("with ").print(pp_logic_decl(arena, attr), decl);
                    });
                }
            }
            Decl::Logic(decls) if decls.iter().all(|d| d.r#type.is_some()) => {
                // functions have a type
                doc.hvbox(2, |doc| {
                    doc.atom("function ")
                        .print(pp_logic_decl(arena, attr), &decls[0]);
                });
                for decl in &decls[1..] {
                    doc.space().hvbox(2, |doc| {
                        doc.atom("with ").print(pp_logic_decl(arena, attr), decl);
                    });
                }
            }
            Decl::Logic(..) => {
                // Mixed predicate/function declarations??
                panic!();
            }
            Decl::Ind(sign, decls) => {
                let keyword = match sign {
                    decl::IndSign::Ind => "inductive",
                    decl::IndSign::Coind => "coinductive",
                };
                doc.hvbox(2, move |doc| {
                    doc.atom_fn(move |f| write!(f, "{keyword} "))
                        .print(pp_ind_decl(arena, attr), &decls[0]);
                    for decl in &decls[1..] {
                        doc.space().hvbox(2, |doc| {
                            doc.atom("with ").print(pp_ind_decl(arena, attr), decl);
                        });
                    }
                });
            }
            Decl::Prop(kind, id, t) => {
                let keyword = match kind {
                    decl::PropKind::Lemma => "lemma",
                    decl::PropKind::Axiom => "axiom",
                    decl::PropKind::Goal => "goal",
                };
                let id = arena.alloc(remove_id_attr("useraxiom", id.clone()));
                doc.hvbox(2, move |doc| {
                    doc.atom_fn(move |f| write!(f, "{keyword} "))
                        .print(pp_id(attr), id)
                        .atom(":")
                        .space()
                        .print(pp_term(arena, attr).marked, t);
                });
            }
            Decl::Let(id, ghost, kind, e) => match &e.desc {
                ExprDesc::Fun(binders, pty_opt, pat, mask, spec, e_) => {
                    pp_let_fun(&pp_expr(arena, attr), arena, attr)(
                        doc,
                        (
                            e.loc,
                            id,
                            *ghost,
                            *kind,
                            (binders, pty_opt, pat, mask, spec, e_),
                        ),
                    )
                }
                ExprDesc::Any(params, kind_, pty_opt, pat, mask, spec) => pp_let_any(arena, attr)(
                    doc,
                    (
                        e.loc,
                        id,
                        *ghost,
                        *kind,
                        (params, *kind_, pty_opt, pat, mask, spec),
                    ),
                ),
                _ => pp_let(&pp_expr(arena, attr), is_ref_expr, arena, attr)(
                    doc,
                    (id, *ghost, *kind, e),
                ),
            },
            Decl::Rec(defs) => {
                doc.vbox(0, |doc| {
                    doc.vbox(2, |doc| {
                        doc.atom("let rec ").print(pp_fundef(arena, attr), &defs[0]);
                    });
                    for def in &defs[1..] {
                        doc.space().hvbox(2, |doc| {
                            doc.atom("with ").print(pp_fundef(arena, attr), def);
                        });
                    }
                });
            }
            Decl::Exn(id, pty, mask) => pp_exn(attr)(doc, (id, pty, mask)),
            Decl::Meta(ident, args) => {
                let pp_metarg = enforce_same_lifetime(|doc, x: &_| {
                    match x {
                        Metarg::Ty(ty) => doc.atom("type ").print(pp_pty(attr).marked, ty),
                        Metarg::Fs(qid) => doc.atom("function ").print(pp_qualid(attr), qid),
                        Metarg::Ps(qid) => doc.atom("predicate ").print(pp_qualid(attr), qid),
                        Metarg::Ax(qid) => doc.atom("axiom ").print(pp_qualid(attr), qid),
                        Metarg::Lm(qid) => doc.atom("lemma ").print(pp_qualid(attr), qid),
                        Metarg::Gl(qid) => doc.atom("goal ").print(pp_qualid(attr), qid),
                        Metarg::Val(qid) => doc.atom("val ").print(pp_qualid(attr), qid),
                        Metarg::Str(s) => doc.quoted(s),
                        Metarg::Int(i) => doc.atom(*i),
                    };
                });
                let pp_args = enforce_same_lifetime(|doc, x| {
                    doc.print_iter(
                        Some(|doc: &mut Doc| {
                            doc.atom(", ");
                        }),
                        pp_metarg,
                        x,
                    );
                });
                doc.atom(r#"meta ""#)
                    .print(pp_id(attr), ident)
                    .atom(r#"" "#)
                    .print(pp_args, args);
            }
            Decl::Cloneexport(_, qid, substs) => {
                doc.hvbox(2, |doc| {
                    doc.atom("clone export ")
                        .print(pp_qualid(attr), qid)
                        .print(pp_substs(attr), substs);
                });
            }
            Decl::Useexport(loc, qid) => {
                doc.hvbox(2, |doc| {
                    doc.print(pp_maybe_marker, *loc)
                        .atom("use export ")
                        .print(pp_qualid(attr), qid);
                });
            }
            Decl::Cloneimport(loc, import, qid, as_id, substs) => {
                doc.hvbox(2, |doc| {
                    doc.print(pp_maybe_marker, *loc)
                        .atom("clone")
                        .print(pp_import, *import)
                        .atom(" ")
                        .print(pp_qualid(attr), qid)
                        .print(
                            pp_opt(
                                {
                                    let mut doc: Doc = Doc::new();
                                    doc.atom(" as ").space();
                                    doc
                                },
                                Doc::default(),
                                Doc::default(),
                                pp_id(attr),
                            ),
                            as_id.as_ref(),
                        )
                        .print(pp_substs(attr), substs);
                });
            }
            Decl::Useimport(loc, import, binds) => {
                let pp_opt_id = || {
                    pp_opt(
                        {
                            let mut doc: Doc = Doc::new();
                            doc.atom(" as ").space();
                            doc
                        },
                        Doc::default(),
                        Doc::default(),
                        pp_id(attr),
                    )
                };
                let pp_bind = |doc: &mut Doc<'a>, (qid, opt_id): &'a (_, Option<_>)| {
                    doc.print(pp_qualid(attr), qid)
                        .print(pp_opt_id(), opt_id.as_ref());
                };
                let pp_binds = pp_print_opt_list(
                    {
                        let mut doc: Doc = Doc::new();
                        doc.atom(" ");
                        doc
                    },
                    Doc::default(),
                    {
                        let mut doc: Doc = Doc::new();
                        doc.atom(", ");
                        doc
                    },
                    Doc::default(),
                    Doc::default(),
                    pp_bind,
                );
                doc.hvbox(2, |doc| {
                    doc.print(pp_maybe_marker, *loc)
                        .atom("use")
                        .print(pp_import, *import)
                        .print(pp_binds, binds);
                });
            }
            Decl::Import(qid) => {
                doc.hvbox(2, |doc| {
                    doc.atom("import ").print(pp_qualid(attr), qid);
                });
            }
            Decl::Scope(loc, export, id, decls) => {
                let pp_export = |doc: &mut Doc| {
                    if *export {
                        doc.atom(" export");
                    }
                };
                doc.vbox(2, |doc| {
                    doc.vbox(2, |doc| {
                        doc.print(pp_maybe_marker, *loc)
                            .atom("scope")
                            .print_(pp_export)
                            .atom(" ")
                            .print(pp_id(attr), id)
                            .space()
                            .print(pp_decls(arena, attr), decls);
                    })
                    .cut()
                    .atom("end");
                });
            }
        }
    }
}

fn pp_decls<'a>(arena: &'a Bump, attr: bool) -> impl Fn(&mut Doc<'a>, &'a [Decl]) {
    move |doc, decls| {
        fn aux(
            decls: &[Decl],
            mut is_first: bool,
            mut previous_was_module: bool,
        ) -> Vec<Option<&Decl>> {
            let mut result = Vec::new();
            for decl in decls {
                let this_is_module = matches!(
                    decl,
                    Decl::Useimport(..)
                        | Decl::Useexport(..)
                        | Decl::Cloneimport(..)
                        | Decl::Cloneexport(..)
                        | Decl::Import(..)
                );
                if !(is_first || (previous_was_module && this_is_module)) {
                    result.push(None);
                }
                result.push(Some(decl));
                is_first = false;
                previous_was_module = this_is_module;
            }
            result
        }

        doc.print_iter(
            Some(|doc: &mut Doc| {
                doc.newline();
            }),
            |doc, x| {
                pp_opt(
                    Doc::default(),
                    Doc::default(),
                    Doc::default(),
                    pp_decl(Some(attr), arena),
                )(doc, x)
            },
            aux(decls, true, false),
        );
    }
}

/// Printer for mlw files
pub fn pp_mlw_file<'a>(attr: Option<bool>, doc: &mut Doc<'a>, arena: &'a Bump, x: &'a MlwFile) {
    let attr = attr.unwrap_or(true);

    match x {
        MlwFile::Decls(decls) => pp_decls(arena, attr)(doc, decls),
        MlwFile::Modules(modules) => {
            let pp_module = |doc: &mut Doc<'a>, (id, decls): &'a (_, Box<_>)| {
                doc.vbox(0, |doc| {
                    doc.vbox(2, |doc| {
                        doc.atom("module ")
                            .print(pp_id(attr), id)
                            .space()
                            .print(pp_decls(arena, attr), decls);
                    })
                    .space()
                    .atom("end");
                });
            };
            let pp_modules = |doc: &mut Doc<'a>, x| {
                doc.print_iter(
                    Some(|doc: &mut Doc| {
                        doc.newline().newline();
                    }),
                    pp_module,
                    x,
                );
            };
            doc.vbox(0, |doc| {
                doc.print(pp_modules, modules);
            });
        }
    }
}
