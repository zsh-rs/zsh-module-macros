//! Shared utilities for parsing macro attribute arguments.
//!
//! Every macro in this crate accepts attribute arguments of the form:
//!
//! ```text
//! #[macro_name("name-literal", key = value, key = value, ...)]
//! ```
//!
//! - The first positional argument is always a name literal — either a Rust
//!   string literal (`"foo"`) or a C-string literal (`c"foo"`). Both normalize
//!   into a `CString`; interior NUL bytes are rejected with a spanned error.
//! - Subsequent arguments are `key = value` pairs in any order.
//! - Each macro picks which keys it accepts; unknown keys are reported with a
//!   span pointing at the offending identifier. Duplicate keys likewise.
//!
//! The [`MinMax`] accumulator handles the `min` / `max` integer keys shared by
//! most macros. Per-macro keys (e.g. `opts` on `#[builtin]`) are handled
//! directly in each macro's `Parse` impl by inspecting `key`/`val` in the
//! [`parse_args`] callback.
//!
//! Signature parsing (extracting the `&Ctx` / `&mut Ctx` from the annotated
//! function) lives in [`extract_ctx_type`].

use std::ffi::CString;

use proc_macro2::{Literal, Span};
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Expr, ExprLit, ExprUnary, FnArg, Ident, ItemFn, Lit, MetaNameValue, Token, Type, UnOp,
};

/// Parse `<name-literal>` optionally followed by `, key = val, ...`.
///
/// Each key/value is dispatched to `handle`. If `handle` returns `Ok(false)`,
/// the key is reported as unknown with a span pointing at the offending ident.
pub fn parse_args<F>(input: ParseStream, mut handle: F) -> syn::Result<Lit>
where
    F: FnMut(&Ident, &Expr) -> syn::Result<bool>,
{
    let name: Lit = input.parse()?;
    if input.is_empty() {
        return Ok(name);
    }
    input.parse::<Token![,]>()?;
    for kv in Punctuated::<MetaNameValue, Token![,]>::parse_terminated(input)? {
        let key = kv
            .path
            .get_ident()
            .ok_or_else(|| syn::Error::new(kv.path.span(), "expected an identifier key"))?;
        if !handle(key, &kv.value)? {
            return Err(syn::Error::new(key.span(), format!("unknown key `{key}`")));
        }
    }
    Ok(name)
}

/// Accumulator for the shared `min` / `max` integer args. Each macro creates
/// one, drains it during kv-dispatch via [`try_take`], then [`resolve`]s with
/// its own defaults.
#[derive(Default)]
pub struct MinMax {
    min: Option<i32>,
    max: Option<i32>,
}

impl MinMax {
    /// Consume a `min` or `max` kv; returns whether `key` was handled.
    pub fn try_take(&mut self, key: &Ident, val: &Expr) -> syn::Result<bool> {
        match key.to_string().as_str() {
            "min" => set_once(&mut self.min, key, val.as_i32()?).map(|_| true),
            "max" => set_once(&mut self.max, key, val.as_i32()?).map(|_| true),
            _ => Ok(false),
        }
    }

    pub fn resolve(self, min_default: i32, max_default: i32) -> (i32, i32) {
        (
            self.min.unwrap_or(min_default),
            self.max.unwrap_or(max_default),
        )
    }
}

pub fn set_once<T>(slot: &mut Option<T>, key: &Ident, value: T) -> syn::Result<()> {
    if slot.is_some() {
        return Err(syn::Error::new(key.span(), format!("duplicate `{key}`")));
    }
    *slot = Some(value);
    Ok(())
}

pub trait LitExt {
    /// Convert a string or c-string literal into an owned `CString`, paired
    /// with the literal's span for downstream diagnostics.
    fn to_cstring(&self) -> syn::Result<(CString, Span)>;

    fn to_cstring_lit(&self) -> syn::Result<Literal> {
        let (cstr, span) = self.to_cstring()?;
        let mut lit = Literal::c_string(&cstr);
        lit.set_span(span);
        Ok(lit)
    }
}

impl LitExt for Lit {
    fn to_cstring(&self) -> syn::Result<(CString, Span)> {
        match self {
            Lit::Str(s) => CString::new(s.value())
                .map(|c| (c, s.span()))
                .map_err(|_| syn::Error::new(s.span(), "string contains interior nul byte")),
            Lit::CStr(c) => Ok((c.value().to_owned(), c.span())),
            _ => Err(syn::Error::new(
                self.span(),
                "expected string or c-string literal",
            )),
        }
    }
}

pub trait ExprExt {
    /// Borrow the underlying [`Lit`] if `self` is a literal expression.
    fn as_lit(&self) -> syn::Result<&Lit>;
    /// Evaluate a positive or negated integer literal as `i32`.
    fn as_i32(&self) -> syn::Result<i32>;
}

impl ExprExt for Expr {
    fn as_lit(&self) -> syn::Result<&Lit> {
        match self {
            Expr::Lit(ExprLit { lit, .. }) => Ok(lit),
            _ => Err(syn::Error::new(self.span(), "expected a literal value")),
        }
    }

    fn as_i32(&self) -> syn::Result<i32> {
        match self {
            Expr::Lit(ExprLit { lit: Lit::Int(i), .. }) => i.base10_parse::<i32>(),
            Expr::Unary(ExprUnary { op: UnOp::Neg(_), expr, .. }) => {
                if let Expr::Lit(ExprLit { lit: Lit::Int(i), .. }) = &**expr {
                    Ok(-i.base10_parse::<i32>()?)
                } else {
                    Err(syn::Error::new(expr.span(), "expected integer literal"))
                }
            }
            _ => Err(syn::Error::new(self.span(), "expected integer literal")),
        }
    }
}

/// The `&Ctx` / `&mut Ctx` argument pulled off the first position of a macro'd
/// function's signature.
pub struct CtxType {
    pub ident: Ident,
    /// `true` when the user wrote `&mut Ctx`; controls whether `#[builtin]`
    /// dispatches to `Features::builtin_mut` or `Features::builtin`.
    pub is_mut: bool,
}

/// Pull the state type out of the annotated function's first argument.
///
/// Returns an [`Err`] (spanned at the offending arg, or the fn ident if there
/// is no arg) when the signature doesn't start with `&Ctx` / `&mut Ctx`.
pub fn extract_ctx_type(input: &ItemFn) -> syn::Result<CtxType> {
    let Some(arg) = input.sig.inputs.first() else {
        return Err(syn::Error::new(
            input.sig.ident.span(),
            "expected a `&Ctx` or `&mut Ctx` as the first argument",
        ));
    };

    if let FnArg::Typed(pat_type) = arg
        && let Type::Reference(type_ref) = &*pat_type.ty
        && let Type::Path(type_path) = &*type_ref.elem
        && let Some(ident) = type_path.path.get_ident()
    {
        return Ok(CtxType {
            ident: ident.clone(),
            is_mut: type_ref.mutability.is_some(),
        });
    }

    Err(syn::Error::new(
        arg.span(),
        "first argument must be a reference to a state type (`&Ctx` or `&mut Ctx`)",
    ))
}
