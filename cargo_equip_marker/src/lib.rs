//! Provides a marker attribute for `cargo-equip`.

/// Returns `item` as is.
#[proc_macro_attribute]
pub fn equip(_: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    item
}
