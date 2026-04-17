//! # Debug flag handling
//!
//! * [`Flag`]
//! * [`register_flag`]
//!
//! Return the state of a flag.
//!
//! * [`test_flag`]

use std::{cell::LazyCell, collections::HashMap, sync::Mutex};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Flag {
    name: &'static str,
    value: bool,
}

static FLAG_TABLE: Mutex<LazyCell<HashMap<&str, (Flag, bool, ocaml_format::DocSync)>>> =
    Mutex::new(LazyCell::new(|| HashMap::with_capacity(17)));

fn gen_register_flag(desc: ocaml_format::DocSync<'static>, s: &'static str, info: bool) -> Flag {
    FLAG_TABLE
        .lock()
        .unwrap()
        .entry(s)
        .or_insert((
            Flag {
                name: s,
                value: false,
            },
            info,
            desc,
        ))
        .0
}

/// Return the corresponding flag, after registering it if needed.
pub fn register_flag(s: &'static str, desc: ocaml_format::DocSync<'static>) -> Flag {
    gen_register_flag(desc, s, false)
}

pub fn test_flag(s: Flag) -> bool {
    s.value
}
