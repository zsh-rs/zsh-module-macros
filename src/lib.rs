use proc_macro::TokenStream;
use syn::{DeriveInput, ItemFn, ItemStruct, parse_macro_input};

// helpers
mod utils;

// features
mod builtin;
mod state;

// derives
mod derive;

/// Declares a module-state struct.
///
/// Annotates a `struct` as the per-module state that `#[builtin]`-registered
/// functions receive a `&mut` reference to.
///
/// # Example
///
/// ```ignore
/// #[state]
/// struct Greeter {
///     count: u32,
/// }
/// ```
///
/// # Expansion (sketch, pre-hygiene)
///
/// The user's struct is re-emitted unchanged, plus a private companion module
/// holding the linkme-registered state container and the `extern "Rust"`
/// trampoline that `#[builtin]` wrappers call into:
///
/// ```ignore
/// struct Greeter { count: u32 }
///
/// mod __zmod_greeter {
///     use super::Greeter;
///     use ::zsh_module::__ as zmod;
///     use zmod::linkme;
///
///     impl zmod::module::SizedModuleState for Greeter {}
///     static MODULE_CONTAINER: Container<Greeter> = Container::new();
///
///     #[linkme::distributed_slice(zmod::CONTAINERS)]
///     static MODULE_VTABLE: &'static dyn ContainerHooks = &MODULE_CONTAINER;
///
///     #[unsafe(no_mangle)]
///     extern "Rust" fn trampoline(cb: TrampolineCallback<Greeter>) -> i32 { /* ... */ }
/// }
/// ```
#[proc_macro_attribute]
pub fn state(_: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    state::state_impl(input).into()
}

/// Registers a zsh builtin command.
///
/// The annotated function's first argument must be `&mut <State>` (or `&<State>`)
/// where `<State>` is the type annotated with [`macro@state`].
///
/// # Arguments
///
/// | Key     | Required | Default | Type            | Meaning                                     |
/// |---------|----------|---------|-----------------|---------------------------------------------|
/// | *name*  | yes      | —       | `"..."` / `c"..."` | builtin name (first positional arg)       |
/// | `min`   | no       | `0`     | integer literal | minimum positional args zsh will accept     |
/// | `max`   | no       | `-1`    | integer literal | maximum positional args (`-1` = unlimited)  |
/// | `opts`  | no       | `c""`   | `"..."` / `c"..."` | getopts-style option spec                |
///
/// # Example
///
/// ```ignore
/// #[builtin("greet", min = 0, max = 3, opts = "fl:")]
/// fn greet(ctx: &mut Greeter, name: &CStr, args: &[&CStr], opts: &Flags) -> Result<()> {
///     println!("hello, {}!", ctx.who);
///     Ok(())
/// }
/// ```
///
/// # Expansion (sketch, pre-hygiene)
///
/// The user's function is re-emitted unchanged; a private companion module
/// holds an `extern "C"` wrapper conforming to zsh's builtin ABI and the
/// `linkme` slice entry that registers it:
///
/// ```ignore
/// fn greet(/* original signature */) -> Result<()> { /* original body */ }
///
/// mod __zmod_greet {
///     use super::{greet, Greeter};
///     use ::zsh_module::__ as zmod;
///
///     extern "C" fn wrapper(name: *mut c_char, args: *mut *mut c_char,
///                           opts: *mut zmod::zsh::options, _id: i32) -> i32 { /* ... */ }
///
///     #[linkme::distributed_slice(zmod::BUILTINS)]
///     static BUILTIN_ENTRY: zmod::zsh::builtin =
///         zmod::zsh::builtin::new(c"greet", wrapper, 0, 3, 0, c"fl:");
/// }
/// ```
#[proc_macro_attribute]
pub fn builtin(args: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let args = parse_macro_input!(args as builtin::BuiltinArgs);
    builtin::builtin_impl(args, input).into()
}


#[proc_macro_derive(Activate)]
pub fn activate_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive::activate_derive_impl(input).into()
}

#[proc_macro_derive(Deactivate)]
pub fn deactivate_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    derive::deactivate_derive_impl(input).into()
}