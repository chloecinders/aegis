use proc_macro::TokenStream;

mod command;
mod meta;
mod slash;

#[proc_macro_attribute]
pub fn command(attr: TokenStream, item: TokenStream) -> TokenStream {
    command::expand(attr, item)
}

#[proc_macro_attribute]
pub fn slash(attr: TokenStream, item: TokenStream) -> TokenStream {
    slash::expand(attr, item)
}

#[proc_macro]
pub fn meta(item: TokenStream) -> TokenStream {
    meta::expand(item)
}
