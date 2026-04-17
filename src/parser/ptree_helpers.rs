//! # Helpers for constructing program with the parse tree API
//!
//! ## identifiers
//!
//! * [`ident`]
//! * [`qualid`]
//! * [`const`]
//! * [`unit_binder`]
//! * [`one_binder`]
//!
//! ## terms and formulas
//!
//! * [`term`]
//! * [`tconst`]
//! * [`tvar`]
//! * [`tapp`]
//! * [`pat`]
//! * [`pat_var`]
//!
//! ## program expressions
//!
//! * [`BREAK_ID`]
//! * [`CONTINUE_ID`]
//! * [`RETURN_ID`]
//! * [`expr`](expr())
//! * [`econst`]
//! * [`eapp`]
//! * [`eapply`]
//! * [`evar`]
//! * [`empty_spec`]
//!
//! ## declarations
//!
//! * [`use`]
//! * [`global_var_decl`]
//!
//! ## Declarations in top-down style
//!
//! The following helpers allows one to create modules, declarations
//!    inside modules, and program functions in a top-down style, instead
//!    of the bottom-up style above
//!
//! This extra layer comes in only one flavor, an imperative one.
//!
//! * [`State`]

use crate::{constant, decl, expr, ity, loc, number, pmodule, ptree::*};

use std::mem;

/// Produces the identifier named `s`
///    optionally with the given attributes and source location
pub fn ident(attrs: Option<Box<[Attr]>>, loc: loc::Position, s: Box<str>) -> Ident {
    let attrs = attrs.unwrap_or(Box::new([]));

    Ident {
        str: s,
        ats: attrs,
        loc,
    }
}

/// Produces the qualified identifier given by the path
///    `l`, a list in the style of `Box::new(["int".into(), "Int".into()])`
pub fn qualid(l: Box<[Box<str>]>) -> Qualid {
    Qualid(
        l.into_iter()
            .map(|x| ident(None, loc::Position::default(), x))
            .collect(),
    )
}

pub fn r#const(kind: Option<number::IntLiteralKind>, i: isize) -> constant::Constant {
    let kind = kind.unwrap_or(number::IntLiteralKind::Dec);

    constant::Constant::Int(number::IntConstant {
        kind,
        int: i.into(),
    })
}

pub fn unit_binder(loc: loc::Position) -> Box<[Binder]> {
    Box::new([Binder(loc, None, false, Some(Pty::Tuple(Box::new([]))))])
}

pub fn one_binder(
    loc: loc::Position,
    ghost: Option<bool>,
    pty: Option<Pty>,
    id: Box<str>,
) -> Box<[Binder]> {
    let ghost = ghost.unwrap_or(false);

    Box::new([Binder(loc, Some(ident(None, loc, id)), ghost, pty)])
}

pub fn term(loc: loc::Position, t: TermDesc) -> Term {
    Term { desc: t, loc }
}

pub fn tvar(loc: loc::Position, id: Qualid) -> Term {
    term(loc, TermDesc::Ident(id))
}

pub fn tapp(loc: loc::Position, f: Qualid, l: Box<[Term]>) -> Term {
    term(loc, TermDesc::Idapp(f, l))
}

pub fn pat(loc: loc::Position, p: PatDesc) -> Pattern {
    Pattern { desc: p, loc }
}

pub fn pat_var(loc: loc::Position, id: Ident) -> Pattern {
    pat(loc, PatDesc::Var(id))
}

pub fn tconst(loc: loc::Position, i: isize) -> Term {
    term(loc, TermDesc::Const(r#const(None, i)))
}

pub const BREAK_ID: &str = "'Break";

pub const CONTINUE_ID: &str = "'Continue";

pub const RETURN_ID: &str = "'Return";

pub fn expr(loc: loc::Position, e: ExprDesc) -> Expr {
    Expr { desc: e, loc }
}

pub fn econst(loc: loc::Position, i: isize) -> Expr {
    expr(loc, ExprDesc::Const(r#const(None, i)))
}

pub fn eapp(loc: loc::Position, f: Qualid, l: Box<[Expr]>) -> Expr {
    expr(loc, ExprDesc::Idapp(f, l))
}

pub fn eapply(loc: loc::Position, e1: Expr, e2: Expr) -> Expr {
    expr(loc, ExprDesc::Apply(Box::new(e1), Box::new(e2)))
}

pub fn evar(loc: loc::Position, x: Qualid) -> Expr {
    expr(loc, ExprDesc::Ident(x))
}

pub fn empty_spec() -> Spec {
    Spec {
        pre: Box::new([]),
        post: Box::new([]),
        xpost: Box::new([]),
        reads: Box::new([]),
        writes: Box::new([]),
        alias: Box::new([]),
        variant: Box::new([]),
        checkrw: false,
        diverge: false,
        partial: false,
    }
}

/// Produces the equivalent of `"use (import) path"` where `path` is denoted by `l`
pub fn r#use(loc: loc::Position, l: Box<[Box<str>]>, import: bool) -> Decl {
    let qid_id_opt = (qualid(l), None);
    Decl::Useimport(loc, import, Box::new([qid_id_opt]))
}

/// Declares a global mutable variable `id` of
///     type `ty`. It returns only the declaration itself
pub fn global_var_decl(ty: Pty, id: Box<str>) -> Decl {
    let v = ExprDesc::Any(
        Box::new([]),
        expr::RsKind::None,
        Some(ty),
        pat(loc::Position::default(), PatDesc::Wild),
        ity::Mask::Visible,
        empty_spec(),
    );
    let body = expr(
        loc::Position::default(),
        ExprDesc::Apply(
            Box::new(expr(loc::Position::default(), ExprDesc::Ref)),
            Box::new(expr(loc::Position::default(), v)),
        ),
    );
    let attrs = Box::new([Attr::Str(pmodule::REF_ATTR.clone())]);
    let id_x = ident(Some(attrs), loc::Position::default(), id);
    Decl::Let(id_x, false, expr::RsKind::None, Box::new(body))
}

fn prop(k: decl::PropKind, loc: loc::Position, id: Box<str>, t: Term) -> Decl {
    Decl::Prop(k, ident(None, loc, id), t)
}

/// Extra helpers for creating declarations in top-down style,
///     imperative interface.
pub struct State {
    modules: Vec<(Ident, Box<[Decl]>)>,
    module_id: Option<Ident>,
    decls: Vec<Decl>,
    fun_head: Option<(bool, bool, Option<Pty>, Ident, Box<[Binder]>)>,
    spec_pre: Vec<Term>,
    spec_writes: Vec<Term>,
    spec_post: Vec<Term>,
}

impl State {
    pub fn create() -> State {
        State {
            modules: Vec::new(),
            module_id: None,
            decls: Vec::new(),
            fun_head: None,
            spec_pre: Vec::new(),
            spec_writes: Vec::new(),
            spec_post: Vec::new(),
        }
    }

    pub fn begin_module(&mut self, loc: loc::Position, name: Box<str>) {
        match (&self.fun_head, &self.module_id, self.decls.as_slice()) {
            (Some(_), _, _) => panic!("begin_module: function declaration already in progress"),
            (None, Some(_), _) => panic!("begin_module: module declaration already in progress"),
            (None, None, []) => {
                let id = ident(None, loc, name);
                self.module_id = Some(id);
            }
            (None, None, _) => panic!("begin_module: top level declarations already in progress"),
        }
    }

    /// see `use_import`
    pub fn r#use(&mut self, loc: loc::Position, l: Box<[Box<str>]>, import: bool) {
        match &self.fun_head {
            Some(_) => panic!("use: function declaration already in progress"),
            None => {
                let d = r#use(loc, l, import);
                self.decls.push(d);
            }
        }
    }

    pub fn add_prop(&mut self, k: decl::PropKind, loc: loc::Position, id: Box<str>, t: Term) {
        match &self.fun_head {
            Some(_) => panic!("add_prop: function declaration already in progress"),
            None => {
                let d = prop(k, loc, id, t);
                self.decls.push(d);
            }
        }
    }

    pub fn add_global_var_decl(&mut self, ty: Pty, id: Box<str>) {
        match &self.fun_head {
            Some(_) => panic!("begin_let: function declaration already in progress"),
            None => {
                let d = global_var_decl(ty, id);
                self.decls.push(d);
            }
        }
    }

    pub fn begin_let(
        &mut self,
        ghost: Option<bool>,
        diverges: Option<bool>,
        ret_type: Option<Pty>,
        id: Box<str>,
        params: Box<[Binder]>,
    ) {
        let ghost = ghost.unwrap_or(false);
        let diverges = diverges.unwrap_or(false);

        match &self.fun_head {
            Some(_) => panic!("begin_let: function declaration already in progress"),
            None => {
                self.fun_head = Some((
                    ghost,
                    diverges,
                    ret_type,
                    ident(None, loc::Position::default(), id),
                    params,
                ))
            }
        }
    }

    pub fn add_pre(&mut self, t: Term) {
        match &self.fun_head {
            None => panic!("add_pre: no function declaration in progress"),
            Some(_) => self.spec_pre.push(t),
        }
    }

    // Useless
    pub fn add_writes<I: IntoIterator<Item = Term>>(&mut self, w: I) {
        match &self.fun_head {
            None => panic!("add_pre: no function declaration in progress"),
            Some(_) => self.spec_writes.extend(w),
        }
    }

    pub fn add_post(&mut self, t: Term) {
        match &self.fun_head {
            None => panic!("add_post: no function declaration in progress"),
            Some(_) => self.spec_post.push(t),
        }
    }

    pub fn add_body(&mut self, e: Expr) {
        match self.fun_head.take() {
            None => panic!("add_body: no function declaration in progress"),
            Some((ghost, diverges, ret_type, id, params)) => {
                let pres = mem::take(&mut self.spec_pre);
                let posts = mem::take(&mut self.spec_post).into_iter().map(|t| {
                    Post(
                        *loc::DUMMY_POSITION,
                        Box::new([(pat(loc::Position::default(), PatDesc::Wild), t)]) as Box<[_]>,
                    )
                });
                let spec = Spec {
                    pre: pres.into(),
                    post: posts.collect(),
                    xpost: Box::new([]),
                    reads: Box::new([]),
                    writes: Box::new([]),
                    alias: Box::new([]),
                    variant: Box::new([]),
                    checkrw: false,
                    diverge: diverges,
                    partial: false,
                };
                let f = ExprDesc::Fun(
                    params,
                    ret_type,
                    pat(loc::Position::default(), PatDesc::Wild),
                    ity::Mask::Visible,
                    spec,
                    Box::new(e),
                );
                let d = Decl::Let(
                    id,
                    ghost,
                    expr::RsKind::None,
                    Box::new(expr(loc::Position::default(), f)),
                );
                self.decls.push(d);
            }
        }
    }

    pub fn end_module(&mut self) {
        match (&self.fun_head, &self.module_id) {
            (Some(_), _) => panic!("end_module: function declaration in progress"),
            (None, None) => panic!("end_module: no module declaration in progress"),
            (None, Some(_)) => {
                let id = self.module_id.take().unwrap();
                let decls = mem::take(&mut self.decls);
                self.modules.push((id, decls.into()));
            }
        }
    }

    pub fn get_mlw_file(self) -> MlwFile {
        match (
            self.fun_head,
            self.module_id,
            self.modules.as_slice(),
            self.decls.as_slice(),
        ) {
            (Some(_), _, _, _) => panic!("get_mlw_file: function declaration in progress"),
            (None, Some(_), _, _) => panic!("get_mlw_file: module declaration in progress"),
            (None, None, _, []) => MlwFile::Modules(self.modules.into()),
            (None, None, [], _) => MlwFile::Decls(self.decls.into()),
            (None, None, _, _) => panic!(),
        }
    }
}
