//! Provides a marker attribute for `cargo-equip`.

#![forbid(unsafe_code)]
#![warn(rust_2018_idioms)]

#[allow(unused_extern_crates)]
extern crate proc_macro; // for compatibility

/// Returns `item` as is.
#[proc_macro_attribute]
pub fn equip(_: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    item
}
