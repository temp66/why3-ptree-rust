//! # WhyML program modules
//!
//! * [`REF_ATTR`]

use crate::ident;

use std::sync::LazyLock;

pub static REF_ATTR: LazyLock<ident::Attribute> =
    LazyLock::new(|| ident::create_attribute("mlw:reference_var".into()));
