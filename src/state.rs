use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemStruct;

use crate::utils::emit::priv_mod;

pub fn state_impl(input: ItemStruct) -> TokenStream {
    let struct_name = &input.ident;
    let priv_mod = priv_mod(struct_name);

    quote! {
        // Re-emit the user's struct unchanged.
        #input

        mod #priv_mod {
            use super::#struct_name;

            #[allow(deprecated)]
            use ::zsh_module::__ as zmod;
            use zmod::linkme;

            use zmod::module::{Container, ContainerHooks, TrampolineCallback};
            impl zmod::module::SizedModuleState for #struct_name {}

            static MODULE_CONTAINER: Container<#struct_name> = Container::new();

            #[linkme::distributed_slice(zmod::CONTAINERS)]
            #[linkme(crate = zmod::linkme)]
            static MODULE_VTABLE: &'static dyn ContainerHooks = &MODULE_CONTAINER;

            fn use_module<F>(applicator: F) -> i32
            where
                F: FnOnce(&mut #struct_name) -> ::zsh_module::types::result::Result<()> + std::panic::UnwindSafe,
            {
                MODULE_CONTAINER.with_state(applicator).unwrap_or(65)
            }

            #[unsafe(no_mangle)]
            extern "Rust" fn trampoline(cb: TrampolineCallback<#struct_name>) -> i32 {
                use_module(cb)
            }
        }
    }
}
