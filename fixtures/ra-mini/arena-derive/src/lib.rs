//! `#[derive(Label)]`: `impl <Name> { pub fn label(&self) -> &'static str { "<Name>" } }`.

use proc_macro::{TokenStream, TokenTree};

#[proc_macro_derive(Label)]
pub fn derive_label(input: TokenStream) -> TokenStream {
    let mut tokens = input.into_iter();
    let mut name = None;
    while let Some(token) = tokens.next() {
        if let TokenTree::Ident(ident) = &token
            && ident.to_string() == "struct"
            && let Some(TokenTree::Ident(next)) = tokens.next()
        {
            name = Some(next.to_string());
            break;
        }
    }
    let name = name.unwrap_or_else(|| "Unknown".to_owned());
    format!("impl {name} {{ pub fn label(&self) -> &'static str {{ \"{name}\" }} }}")
        .parse()
        .unwrap_or_default()
}
