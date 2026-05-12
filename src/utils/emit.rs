//! Codegen helpers shared by every macro in this crate.

use proc_macro2::Ident;
use quote::format_ident;

/// Build the name of the private companion module emitted by each macro:
/// `Greeter` → `__zmod_greeter`, `greet` → `__zmod_greet`.
///
/// All macros wrap their generated items inside a `mod __zmod_<name>` so that
/// the linkme registration and trampoline glue stays out of the user's
/// namespace.
pub fn priv_mod(ident: &Ident) -> Ident {
    format_ident!("__zmod_{}", ident.to_string().to_lowercase())
}
