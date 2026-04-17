//! # Parse trees
//!
//! The module provides datatypes for WhyML parse trees.
//!
//! These datatypes are produced by the WhyML parser module `parser`.
//!
//! They can be alternatively produced via code, and processed later
//!    on by typing module `typing`. See also Section 4.9. "ML Programs"
//!    of the documentation.
//!
//! ## Identifiers and attributes
//!
//! * [`Attr`]
//! * [`Ident`]
//! * [`Qualid`]
//!
//! ## Types
//!
//! * [`Pty`]
//!
//! ## Patterns
//!
//! * [`Ghost`]
//! * [`Pattern`]
//! * [`PatDesc`]
//!
//! ## Logical terms and formulas
//!
//! * [`Binder`]
//! * [`Param`]
//! * [`Term`]
//! * [`TermDesc`]
//!
//! ## Program expressions
//!
//! * [`Invariant`]
//! * [`Variant`]
//! * [`Pre`]
//! * [`Post`]
//! * [`Xpost`]
//! * [`Spec`]
//! * [`Expr`]
//! * [`ExprDesc`]
//! * [`RegBranch`]
//! * [`ExnBranch`]
//! * [`Fundef`]
//!
//! ## Declarations
//!
//! * [`Field`]
//! * [`TypeDef`]
//! * [`Visibility`]
//! * [`TypeDecl`]
//! * [`LogicDecl`]
//! * [`IndDecl`]
//! * [`Metarg`]
//! * [`CloneSubst`]
//! * [`Decl`]
//! * [`MlwFile`]

use crate::{constant, decl, dterm, expr, ident, ity, loc};

use malachite::Integer;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// attributes, with a specific case for a source location
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Attr {
    Str(ident::Attribute),
    Pos(loc::Position),
}

/// identifiers, with attributes and a source location
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Ident {
    pub str: Box<str>,
    pub ats: Box<[Attr]>,
    pub loc: loc::Position,
}

/// qualified identifiers
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Qualid(pub Box<[Ident]>);

/// type expressions
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Pty {
    /// type variable
    Tyvar(Ident),
    /// type constructor, possibly with arguments, e.g., `int`, `list bool`, etc.
    Tyapp(Qualid, Box<[Pty]>),
    /// tuples, e.g., `(int,bool)`
    Tuple(Box<[Pty]>),
    /// reference type, e.g., `ref int`, as used by the "auto-dereference"
    ///    mechanism (See manual Section 13.1. "Release Notes for version
    ///    1.2: new syntax for auto-dereference")
    Ref(Box<[Pty]>),
    /// arrow type, e.g., `int -> bool`
    Arrow(Box<Pty>, Box<Pty>),
    /// opening scope locally, e.g., `M.((list t,u))`
    Scope(Qualid, Box<Pty>),
    /// parenthesised type
    Paren(Box<Pty>),
    /// purify a type
    Pure(Box<Pty>),
}

/// "ghost" modifier
pub type Ghost = bool;

/// Patterns, equipped with a source location
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Pattern {
    pub desc: PatDesc,
    pub loc: loc::Position,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum PatDesc {
    /// wildcard, that is "_"
    Wild,
    /// variable as a pattern
    Var(Ident),
    /// constructor pattern, e.g., `Cons(x,y)`
    App(Qualid, Box<[Pattern]>),
    /// record pattern
    Rec(Box<[(Qualid, Pattern)]>),
    /// tuple pattern
    Tuple(Box<[Pattern]>),
    /// as-pattern, e.g., `Cons(x,y) as z`
    As(Box<Pattern>, Ident, Ghost),
    /// or-pattern `p1 | p2`
    Or(Box<Pattern>, Box<Pattern>),
    /// type cast
    Cast(Box<Pattern>, Pty),
    /// open scope locally
    Scope(Qualid, Box<Pattern>),
    /// parenthesised pattern
    Paren(Box<Pattern>),
    /// explicitly ghost pattern
    Ghost(Box<Pattern>),
}

/// binder as 4-uple `(loc, id, ghost, type)` to represent "ghost? id? :
///    type?". `id` and `type` cannot be `None` at the same time
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Binder(
    pub loc::Position,
    pub Option<Ident>,
    pub Ghost,
    pub Option<Pty>,
);

/// parameter as 4-uple `(loc, id, ghost, type)` to represent
///    "ghost? id? : type".
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Param(pub loc::Position, pub Option<Ident>, pub Ghost, pub Pty);

/// Terms, equipped with a source location
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Term {
    pub desc: TermDesc,
    pub loc: loc::Position,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TermDesc {
    /// the true proposition
    True,
    /// the false proposition
    False,
    /// constant literals
    Const(constant::Constant),
    /// identifiers
    Ident(Qualid),
    /// identifier as reference, e.g., `&x` (See manual Section
    ///    13.1. "Release Notes for version 1.2: new syntax for
    ///    auto-dereference")
    Asref(Qualid),
    /// (first-order) application of a logic identifier to a list of terms
    Idapp(Qualid, Box<[Term]>),
    /// curried application, of a term to a term
    Apply(Box<Term>, Box<Term>),
    /// application of a binary operation in an infix fashion, allowing chaining.
    ///     For example, `Infix(t1, "<=", Infix(t2, "<", t3))` denotes
    ///     `t1 <= t2 /\ t2 < t3`
    Infix(Box<Term>, Ident, Box<Term>),
    /// application of a binary operation in an infix style, but without chaining
    Innfix(Box<Term>, Ident, Box<Term>),
    /// application of a binary logic connective, in an infix fashion, allowing
    ///     chaining. For example, `Binop(p1, "<->", Binop(p2, "<->", p3))` denotes
    ///     `(p1 <-> p2) /\ (p2 <-> p3)`
    Binop(Box<Term>, dterm::Dbinop, Box<Term>),
    /// application of a binary logic connective, but without chaining
    Binnop(Box<Term>, dterm::Dbinop, Box<Term>),
    /// logic negation
    Not(Box<Term>),
    /// if-expression
    If(Box<Term>, Box<Term>, Box<Term>),
    /// quantified formulas. The third argument is a list of triggers.
    Quant(dterm::Dquant, Box<[Binder]>, Box<[Box<[Term]>]>, Box<Term>),
    /// `Eps(x, ty, f)` denotes the epsilon term "any `x` of type `ty`
    ///    that satisfies `f`".  Use with caution since if there is no such
    ///    `x` satisfying `f`, then it acts like introducing an inconsistent
    ///    axiom. (As a matter of fact, this is the reason why there is no
    ///    concrete syntax for such epsilon-terms.)
    Eps(Ident, Pty, Box<Term>),
    /// term annotated with an attribute
    Attr(Attr, Box<Term>),
    /// let-expression
    Let(Ident, Box<Term>, Box<Term>),
    /// pattern-matching
    Case(Box<Term>, Box<[(Pattern, Term)]>),
    /// type casting
    Cast(Box<Term>, Pty),
    /// tuples
    Tuple(Box<[Term]>),
    /// record expressions
    Record(Box<[(Qualid, Term)]>),
    /// record update expression
    Update(Box<Term>, Box<[(Qualid, Term)]>),
    /// local scope
    Scope(Qualid, Box<Term>),
    /// "at" modifier. The "old" modifier is a particular case with
    ///    the identifier `dexpr::OLD_LABEL`
    At(Box<Term>, Ident),
}

/// Loop invariant or type invariant
pub type Invariant = [Term];

/// Variant for both loops and recursive functions. The option
///    identifier is an optional ordering predicate
pub type Variant = [(Term, Option<Qualid>)];

/// Precondition
pub type Pre = Term;

/// Normal postcondition
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Post(pub loc::Position, pub Box<[(Pattern, Term)]>);

/// Exceptional postconditions
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Xpost(
    pub loc::Position,
    pub Box<[(Qualid, Option<(Pattern, Term)>)]>,
);

/// Contract
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Spec {
    /// preconditions
    pub pre: Box<[Pre]>,
    /// normal postconditions
    pub post: Box<[Post]>,
    /// exceptional postconditions
    pub xpost: Box<[Xpost]>,
    /// "reads" clause
    pub reads: Box<[Qualid]>,
    /// "writes" clause
    pub writes: Box<[Term]>,
    /// "alias" clause
    pub alias: Box<[(Term, Term)]>,
    /// variant for recursive functions
    pub variant: Box<Variant>,
    /// should the reads and writes clauses be checked against the given body?
    pub checkrw: bool,
    /// may the function diverge?
    pub diverge: bool,
    /// is the function partial?
    pub partial: bool,
}

/// Expressions, equipped with a source location
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Expr {
    pub desc: ExprDesc,
    pub loc: loc::Position,
}

/// Expression kinds
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ExprDesc {
    /// built-in operator `ref` for auto-dereference syntax. (See manual Section
    ///    13.1. "Release Notes for version 1.2: new syntax for
    ///    auto-dereference")
    Ref,
    /// Boolean literal `True`
    True,
    /// Boolean literal `False`
    False,
    /// Constant literals
    Const(constant::Constant),
    /// Variable identifier
    Ident(Qualid),
    /// identifier as reference, e.g., `&x`  (See manual Section
    ///    13.1. "Release Notes for version 1.2: new syntax for
    ///    auto-dereference")
    Asref(Qualid),
    /// Uncurried application of a function identifier to a list of arguments
    Idapp(Qualid, Box<[Expr]>),
    /// Curried application
    Apply(Box<Expr>, Box<Expr>),
    /// application of a binary function identifier, in an infix fashion, allowing
    ///    chaining, e.g., `Infix(e1, "<=", Infix(e2, "<", e3))` denotes
    ///    `e1 <= e2 && e2 < e3`
    Infix(Box<Expr>, Ident, Box<Expr>),
    /// application of a binary function, but without chaining
    Innfix(Box<Expr>, Ident, Box<Expr>),
    /// `let ... in ...` expression
    Let(Ident, Ghost, expr::RsKind, Box<Expr>, Box<Expr>),
    /// Local definition of function(s), possibly mutually recursive
    Rec(Box<[Fundef]>, Box<Expr>),
    /// Anonymous function
    Fun(
        Box<[Binder]>,
        Option<Pty>,
        Pattern,
        ity::Mask,
        Spec,
        Box<Expr>,
    ),
    /// "any params : ty \<spec\>": abstract expression with a specification,
    ///     generating a VC for existence
    Any(
        Box<[Param]>,
        expr::RsKind,
        Option<Pty>,
        Pattern,
        ity::Mask,
        Spec,
    ),
    /// Tuple of expressions
    Tuple(Box<[Expr]>),
    /// Record expression, e.g., `{f=e1; g=e2; ...}`
    Record(Box<[(Qualid, Expr)]>),
    /// Record update, e.g., `{e with f=e1; ...}`
    Update(Box<Expr>, Box<[(Qualid, Expr)]>),
    /// Assignment, of a mutable variable (no qualid given) or of a record field (qualid
    /// given). Assignments are possibly in parallel, e.g., `x.f, y.g, z <- e1, e2, e3`
    Assign(Box<[(Expr, Option<Qualid>, Expr)]>),
    /// Sequence of two expressions, the first one being supposed of type unit
    Sequence(Box<Expr>, Box<Expr>),
    /// `if e1 then e2 else e3` expression
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    /// `while` loop with annotations
    While(Box<Expr>, Box<Invariant>, Box<Variant>, Box<Expr>),
    /// lazy conjunction
    And(Box<Expr>, Box<Expr>),
    /// lazy disjunction
    Or(Box<Expr>, Box<Expr>),
    /// Boolean negation
    Not(Box<Expr>),
    /// match expression, including both regular patterns and exception
    ///    patterns (those lists cannot be both empty)
    Match(Box<Expr>, Box<[RegBranch]>, Box<[ExnBranch]>),
    /// `absurd` statement to mark unreachable branches
    Absurd,
    /// turns a logical term into a pure expression, e.g., `pure { t }`
    Pure(Term),
    /// promotes a logic symbol in programs, e.g., `{f}` or `M.{f}`
    Idpur(Qualid),
    /// raise an exception, possibly with an argument
    Raise(Qualid, Option<Box<Expr>>),
    /// local declaration of an exception, e.g., `let exception E in e`
    Exn(Ident, Pty, ity::Mask, Box<Expr>),
    /// local declaration of an exception, implicitly captured. Used by Why3 for handling
    ///     `return`, `break`, and `continue`
    Optexn(Ident, ity::Mask, Box<Expr>),
    /// "for" loops
    For(
        Ident,
        Box<Expr>,
        expr::ForDirection,
        Box<Expr>,
        Box<Invariant>,
        Box<Expr>,
    ),
    /// `assert`, `assume`, and `check` expressions
    Assert(expr::AssertionKind, Term),
    /// open scope locally, e.g., `M.(e)`
    Scope(Qualid, Box<Expr>),
    /// introduction of a label, e.g., `label L in e`
    Label(Ident, Box<Expr>),
    /// cast an expression to a given type, e.g., `(e:ty)`
    Cast(Box<Expr>, Pty),
    /// forces an expression to be ghost, e.g., `ghost e`
    Ghost(Box<Expr>),
    /// attach an attribute to an expression
    Attr(Attr, Box<Expr>),
}

/// A regular match branch
pub type RegBranch = (Pattern, Expr);

/// An exception match branch
pub type ExnBranch = (Qualid, Option<Pattern>, Expr);

/// local function definition
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Fundef(
    pub Ident,
    pub Ghost,
    pub expr::RsKind,
    pub Box<[Binder]>,
    pub Option<Pty>,
    pub Pattern,
    pub ity::Mask,
    pub Spec,
    pub Expr,
);

/// record fields
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Field {
    pub loc: loc::Position,
    pub ident: Ident,
    pub pty: Pty,
    pub mutable: bool,
    pub ghost: bool,
}

/// Type definition body
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum TypeDef {
    /// alias type
    Alias(Pty),
    /// algebraic type
    Algebraic(Box<[(loc::Position, Ident, Box<[Param]>)]>),
    /// record type
    Record(Box<[Field]>),
    /// integer type in given range
    Range(Integer, Integer),
    /// floating-point type with given exponent and precision
    Float(isize, isize),
}

/// The different kinds of visibility
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Visibility {
    Public,
    Private,
    /// = Private + ghost fields
    Abstract,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TypeDecl {
    pub loc: loc::Position,
    pub ident: Ident,
    pub params: Box<[Ident]>,
    /// visibility, for records only
    pub vis: Visibility,
    /// mutability, for records or abstract types
    pub r#mut: bool,
    /// invariant, for records only
    pub inv: Box<Invariant>,
    /// witness for the invariant
    pub wit: Option<Expr>,
    pub def: TypeDef,
}

/// A single declaration of a function or predicate
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LogicDecl {
    pub loc: loc::Position,
    pub ident: Ident,
    pub params: Box<[Param]>,
    pub r#type: Option<Pty>,
    pub def: Option<Term>,
}

/// A single declaration of an inductive predicate
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct IndDecl {
    pub loc: loc::Position,
    pub ident: Ident,
    pub params: Box<[Param]>,
    pub def: Box<[(loc::Position, Ident, Term)]>,
}

/// Arguments of "meta" declarations
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Metarg {
    Ty(Pty),
    Fs(Qualid),
    Ps(Qualid),
    Ax(Qualid),
    Lm(Qualid),
    Gl(Qualid),
    Val(Qualid),
    Str(Box<str>),
    Int(isize),
}

/// The possible "clone" substitution elements
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CloneSubst {
    Tsym(Qualid, Box<[Ident]>, Pty),
    Fsym(Qualid, Qualid),
    Psym(Qualid, Qualid),
    Vsym(Qualid, Qualid),
    Xsym(Qualid, Qualid),
    Prop(decl::PropKind),
    Axiom(Qualid),
    Lemma(Qualid),
    Goal(Qualid),
}

/// top-level declarations
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Decl {
    /// Type declaration
    Type(Box<[TypeDecl]>),
    /// Collection of "function"s and "predicate"s, mutually recursively declared
    Logic(Box<[LogicDecl]>),
    /// An inductive or co-inductive predicate
    Ind(decl::IndSign, Box<[IndDecl]>),
    /// Propositions: "lemma" or "goal" or "axiom"
    Prop(decl::PropKind, Ident, Term),
    // `Expr` is deliberately boxed to reduce the size of the variant.
    /// Global program variable or function
    Let(Ident, Ghost, expr::RsKind, Box<Expr>),
    /// set of program functions, defined mutually recursively
    Rec(Box<[Fundef]>),
    /// Declaration of global exceptions
    Exn(Ident, Pty, ity::Mask),
    /// Declaration of a "meta"
    Meta(Ident, Box<[Metarg]>),
    /// "clone export"
    Cloneexport(loc::Position, Qualid, Box<[CloneSubst]>),
    /// "use export"
    Useexport(loc::Position, Qualid),
    /// "clone import ... as ..."
    Cloneimport(
        loc::Position,
        bool,
        Qualid,
        Option<Ident>,
        Box<[CloneSubst]>,
    ),
    /// "use import ... as ..."
    Useimport(loc::Position, bool, Box<[(Qualid, Option<Ident>)]>),
    /// "import"
    Import(Qualid),
    /// "scope"
    Scope(loc::Position, bool, Ident, Box<[Decl]>),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MlwFile {
    /// a list of modules containing lists of declarations
    Modules(Box<[(Ident, Box<[Decl]>)]>),
    /// a list of declarations outside any module
    Decls(Box<[Decl]>),
}
