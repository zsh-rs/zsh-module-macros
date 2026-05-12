use proc_macro2::{Literal, TokenStream};
use quote::quote;
use syn::ItemFn;
use syn::parse::{Parse, ParseStream};

use crate::utils::args::{
    CtxType, ExprExt, LitExt, MinMax, extract_ctx_type, parse_args, set_once,
};
use crate::utils::emit::priv_mod;

pub struct BuiltinArgs {
    name: Literal,
    min: i32,
    max: i32,
    opts: Literal,
}

impl Parse for BuiltinArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut min_max = MinMax::default();
        let mut opts: Option<Literal> = None;

        let name = parse_args(input, |key, val| {
            if min_max.try_take(key, val)? {
                return Ok(true);
            }
            if key.to_string().as_str() == "opts" {
                let s = val.as_lit()?.to_cstring_lit()?;
                set_once(&mut opts, key, s)?;
                return Ok(true);
            }
            Ok(false)
        })?;

        let name_lit = name.to_cstring_lit()?;
        let (min, max) = min_max.resolve(0, -1);
        let opts_lit = opts.unwrap_or(Literal::c_string(c""));

        Ok(Self {
            name: name_lit,
            min,
            max,
            opts: opts_lit,
        })
    }
}

pub fn builtin_impl(args: BuiltinArgs, input: ItemFn) -> TokenStream {
    let fn_name = &input.sig.ident;
    let CtxType { ident: ctx_type, is_mut } = match extract_ctx_type(&input) {
        Ok(c) => c,
        Err(e) => return e.to_compile_error(),
    };
    let dispatch = if is_mut {
        quote! { s.builtin_mut(#fn_name, name, args, opts) }
    } else {
        quote! { s.builtin(#fn_name, name, args, opts) }
    };

    let BuiltinArgs {
        name,
        min,
        max,
        opts,
    } = args;

    let priv_mod = priv_mod(fn_name);

    quote! {
        #input

        mod #priv_mod {
            use super::#fn_name;
            use super::#ctx_type;

            #[allow(deprecated)]
            use ::zsh_module::__ as zmod;
            use zmod::linkme;
            
            unsafe extern "Rust" {
                unsafe fn trampoline(cb: zmod::module::TrampolineCallback<#ctx_type>) -> i32;
            }
            
            extern "C" fn wrapper(
                name: *mut std::ffi::c_char,
                args: *mut *mut std::ffi::c_char,
                opts: *mut zmod::zsh::options,
                _id: i32,
            ) -> i32 {
                use zmod::features::Features;
                let cb = move |s: &mut #ctx_type| #dispatch;
                unsafe { trampoline(Box::new(cb)) }
            }

            const MAX_ARGS: i32 = #max;
            
            #[linkme::distributed_slice(zmod::BUILTINS)]
            #[linkme(crate = zmod::linkme)]
            static BUILTIN_ENTRY: zmod::zsh::builtin =
                zmod::zsh::builtin::new(#name, wrapper, #min, MAX_ARGS, 0, #opts);
        }
    }
}
