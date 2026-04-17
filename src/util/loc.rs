//! # Source locations
//!
//! ## Locations in files
//!
//! In Why3, source locations represent a part of a file, denoted by a
//! starting point and an end point. Both of these points are
//! represented by a line number and a column number.
//!
//! So far, line numbers start with 1 and column number start with 0.
//! (See [this issue](https://gitlab.inria.fr/why3/why3/-/issues/706).)
//!
//! * [`Position`]
//! * [`user_position`]
//! * [`DUMMY_POSITION`]
//! * [`get`]
//! * [`equal`]
//! * [`pp_position`]
//!
//! ## Located warnings
//!
//! * [`WarningId`]
//! * [`register_warning`]
//! * [`warning`]

use std::{
    cell::LazyCell,
    collections::HashMap,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// To reduce memory consumption, a pair (line,col) is stored in a single
//    int using

//      (line << bits) | col

//   On 32-bits architecture, bits is 12. This will thus support column
//    numbers up to 4095 and line numbers up to 2^19

//   On 64-bits architecture, bits is 16. This will thus support column
//    numbers up to 65535 and line numbers up to 2^47

//   The file names are also hashed to ensure an optimal sharing

mod file_tags {
    use std::{
        cell::LazyCell,
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    static mut TAG_COUNTER: isize = 0;

    static FILE_TAGS: Mutex<LazyCell<HashMap<Box<str>, isize>>> =
        Mutex::new(LazyCell::new(|| HashMap::with_capacity(7)));

    static TAG_TO_FILE: Mutex<LazyCell<HashMap<isize, Arc<str>>>> =
        Mutex::new(LazyCell::new(|| HashMap::with_capacity(7)));

    pub fn get_file_tag(file: &str) -> isize {
        *FILE_TAGS
            .lock()
            .unwrap()
            .entry(file.into())
            .or_insert_with(|| unsafe {
                let n = TAG_COUNTER;
                TAG_TO_FILE.lock().unwrap().insert(n, file.into());
                TAG_COUNTER += 1;
                n
            })
    }

    pub fn tag_to_file(tag: isize) -> Arc<str> {
        TAG_TO_FILE.lock().unwrap().get(&tag).unwrap().clone()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Position {
    file_tag: isize,
    // compressed line/col
    start: isize,
    // compressed line/col
    end: isize,
}

impl Default for Position {
    fn default() -> Self {
        *DUMMY_POSITION
    }
}

const BITS_COL: isize = if usize::BITS == 32 {
    12
} else if usize::BITS == 64 {
    16
} else {
    panic!("word size should be 32 or 64");
};

const MASK_COL: isize = (1 << BITS_COL) - 1;

const MAX_LINE: isize = (1 << (usize::BITS - BITS_COL as u32 - 1)) - 1;

/// Returns the file, the line and character numbers of the
///    beginning and the line and character numbers of the end of the
///    given position.
pub fn get(p: Position) -> (Arc<str>, isize, isize, isize, isize) {
    let f = file_tags::tag_to_file(p.file_tag);
    let b = p.start;
    let e = p.end;
    (f, b >> BITS_COL, b & MASK_COL, e >> BITS_COL, e & MASK_COL)
}

/// Dummy source position.
pub static DUMMY_POSITION: LazyLock<Position> = LazyLock::new(|| {
    let tag = file_tags::get_file_tag("");
    Position {
        file_tag: tag,
        start: 0,
        end: 0,
    }
});

pub fn equal(lhs: Position, rhs: Position) -> bool {
    lhs == rhs
}

fn pp_position_tail(doc: &mut ocaml_format::Doc, bl: isize, bc: isize, el: isize, ec: isize) {
    doc.atom_fn(move |f| write!(f, "line {bl}, character"));
    if bl == el {
        doc.atom_fn(move |f| write!(f, "s {bc}-{ec}"));
    } else {
        doc.atom_fn(move |f| write!(f, " {bc} to line {el}, character {ec}"));
    }
}

/// Formats the position `loc` in the given
/// document, in a human readable way, that is:
/// * either `"filename", line l, characters bc-ec` if the position is on a single line,
/// * or `"filename", line bl, character bc to line el, character ec` if the position is multi-line.
///
/// The file name is not printed if empty.
pub fn pp_position(doc: &mut ocaml_format::Doc, loc: Position) {
    let (f, bl, bc, el, ec) = get(loc);
    if !f.is_empty() {
        doc.atom_fn(move |fmt| write!(fmt, r#""{f}", "#));
    }
    pp_position_tail(doc, bl, bc, el, ec);
}

// warnings

fn default_hook(loc: Option<Position>, s: &str) {
    use ocaml_format::*;

    match loc {
        None => eprintln!(
            "{}",
            (Doc::new() as Doc)
                .atom_fn(|f| write!(f, "Warning: {s}"))
                .display(&FormattingOptions::new())
        ),
        Some(l) => eprintln!(
            "{}",
            (Doc::new() as Doc)
                .atom("Warning, file ")
                .print(pp_position, l)
                .atom_fn(|f| write!(f, ": {s}"))
                .display(&FormattingOptions::new())
        ),
    }
}

static WARNING_HOOK: LazyLock<Box<dyn Fn(Option<Position>, &str) + Send + Sync>> =
    LazyLock::new(|| Box::new(default_hook));

/// warning identifiers
pub type WarningId = &'static str;

struct Warning {
    #[expect(dead_code)]
    descr: ocaml_format::DocSync<'static>,
    enabled: bool,
}

static WARNING_TABLE: Mutex<LazyCell<HashMap<WarningId, Warning>>> =
    Mutex::new(LazyCell::new(|| HashMap::with_capacity(17)));

/// Registers a new warning under the
///    given `name` with the given `desc`ription.
pub fn register_warning(name: WarningId, desc: ocaml_format::DocSync<'static>) -> WarningId {
    WARNING_TABLE
        .lock()
        .unwrap()
        .entry(name)
        .or_insert(Warning {
            descr: desc,
            enabled: true,
        });
    name
}

fn warning_active(id: WarningId) -> bool {
    WARNING_TABLE.lock().unwrap().get(id).unwrap().enabled
}

/// Emits a warning in the given document
///    `doc`. Adds the location `loc` if it is given. Emits nothing if the
///    `id` is given and disabled, with one of the functions below.
pub fn warning(id: WarningId, loc: Option<Position>, doc: ocaml_format::Doc) {
    use ocaml_format::*;

    let mut doc_: Doc = Doc::new();
    let handle = |doc: &mut Doc| {
        let b = format!(
            "{}",
            doc.display(&FormattingOptions::new().set_width(1000000000))
        );
        WARNING_HOOK(loc, &b);
    };
    if warning_active(id) {
        handle(doc_.sbox(0, |doc_| {
            doc_.extend(doc);
        }));
    }
}

// user positions

static WARN_START_OVERFLOW: LazyLock<WarningId> = LazyLock::new(|| {
    register_warning("start_overflow", {
        let mut doc = ocaml_format::DocSync::new();
        doc.atom("Warn when the start character of a source location overflows into the next line");
        doc
    })
});

static WARN_END_OVERFLOW: LazyLock<WarningId> = LazyLock::new(|| {
    register_warning("end_overflow", {
        let mut doc = ocaml_format::DocSync::new();
        doc.atom("Warn when the end character of a source location overflows into the next line");
        doc
    })
});

static WARNING_EMITTED: AtomicBool = AtomicBool::new(false);

/// Builds the source position for file
/// `f`, starting at line `bl` and character `bc` and ending at line
/// `el` and character `ec`.
pub fn user_position(f: &str, bl: isize, bc: isize, el: isize, ec: isize) -> Position {
    if !(0..=MAX_LINE).contains(&bl) {
        panic!("loc::user_position: start line number `{bl}` out of bounds");
    }
    if bc < 0 {
        panic!("loc::user_position: start char number `{bc}` out of bounds");
    }
    if bc > MASK_COL
        && WARNING_EMITTED
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    {
        warning(&WARN_START_OVERFLOW, None, {
            let mut doc: ocaml_format::Doc = ocaml_format::Doc::new();
            doc.atom_fn(|f| {
                write!(
                    f,
                    "loc::user_position: start char number `{bc}` overflows on next line"
                )
            });
            doc
        });
    }
    if !(0..=MAX_LINE).contains(&el) {
        panic!("loc::user_position: end line number `{el}` out of bounds");
    }
    if ec < 0 {
        panic!("loc::user_position: end char number `{ec}` out of bounds");
    }
    if ec >= MASK_COL
        && WARNING_EMITTED
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
    {
        warning(&WARN_END_OVERFLOW, None, {
            let mut doc: ocaml_format::Doc = ocaml_format::Doc::new();
            doc.atom_fn(|f| {
                write!(
                    f,
                    "loc::user_position: end char number `{ec}` overflows on next line"
                )
            });
            doc
        });
    }
    let tag = file_tags::get_file_tag(f);
    Position {
        file_tag: tag,
        start: bl << BITS_COL | bc,
        end: el << BITS_COL | ec,
    }
}
