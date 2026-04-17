//! Helpers for formatted pretty-printing
//!
//! * [`print_option`]
//! * [`comma`]

use ocaml_format::*;

pub fn print_option<'a, T>(f: impl Fn(&mut Doc<'a>, T)) -> impl Fn(&mut Doc<'a>, Option<T>) {
    move |doc, x| {
        if let Some(x) = x {
            f(doc, x);
        }
    }
}

pub fn comma(doc: &mut Doc) {
    doc.atom(",").space();
}
