use syn::DeriveInput;




pub fn activate_derive_impl(input: DeriveInput) -> proc_macro2::TokenStream {
    let name = input.ident;

    quote::quote! {
        impl zsh_module::Activate for #name {
            fn activate(&mut self) -> zsh_module::Result<()> {
                Ok(())
            }
        }
    }
}

pub fn deactivate_derive_impl(input: DeriveInput) -> proc_macro2::TokenStream {
    let name = input.ident;

    quote::quote! {
        impl zsh_module::Deactivate for #name {
            fn deactivate(&mut self) -> zsh_module::Result<()> {
                Ok(())
            }
        }
    }
}