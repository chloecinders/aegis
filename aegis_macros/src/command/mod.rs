pub mod model;

use quote::{ToTokens, quote};
use syn::{Data, DeriveInput, Error, Fields, Ident};

use model::{Modeled, Shape, model};

fn bind(field: &Modeled, followed: bool) -> impl ToTokens {
    let ident = &field.ident;
    let inner = &field.inner;
    let name = &field.name;

    match field.shape {
        Shape::Positional => quote! {
            let #ident = crate::command::value::positional::<#inner>(cx, __stream, #name).await?;
        },
        Shape::Optional if followed => quote! {
            let #ident = crate::command::value::skippable::<#inner>(cx, __stream, #name).await?;
        },
        Shape::Optional => quote! {
            let #ident = crate::command::value::optional::<#inner>(cx, __stream, #name).await?;
        },
        Shape::Rest if field.optional => quote! {
            let #ident = crate::command::value::rest_optional::<#inner>(cx, __stream, #name).await?;
        },
        Shape::Rest => quote! {
            let #ident = crate::command::value::rest::<#inner>(cx, __stream, #name).await?;
        },
        Shape::Reply => quote! {
            let #ident = crate::command::value::reply::<#inner>(cx, __stream, #name).await?;
        },
        Shape::ReplyOptional => quote! {
            let #ident = crate::command::value::reply_optional::<#inner>(cx, __stream, #name).await?;
        },
        Shape::Flag if field.optional => quote! {
            let #ident = crate::command::value::flag::<#inner>(cx, &__flags, #name).await?;
        },
        Shape::Flag => quote! {
            let #ident = crate::command::value::switch(&__flags, #name);
        },
    }
}

fn describe(field: &Modeled) -> impl ToTokens {
    let inner = &field.inner;
    let name = &field.name;
    let shape = field.shape.ident(&field.ident);
    let amend = &field.amend;
    let short = match &field.short {
        Some(letter) => quote! { ::core::option::Option::Some(#letter) },
        None => quote! { ::core::option::Option::None },
    };
    let desc = match &field.desc {
        Some(line) => quote! { #line },
        None => quote! { "" },
    };

    quote! {
        crate::command::args::Field {
            name: #name,
            kind: <#inner as crate::command::value::FromArgs>::KIND,
            shape: crate::command::args::Shape::#shape,
            short: #short,
            desc: #desc,
            amend: crate::domain::action::Amendment::#amend,
        }
    }
}

fn build(input: &DeriveInput) -> syn::Result<impl ToTokens + 'static> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "a command cannot be generic",
        ));
    }

    let Data::Struct(body) = &input.data else {
        return Err(Error::new_spanned(
            input,
            "#[command] is an argument struct",
        ));
    };

    let Fields::Named(declared) = &body.fields else {
        return Err(Error::new_spanned(
            &body.fields,
            "command arguments must be named fields",
        ));
    };

    let fields = declared
        .named
        .iter()
        .map(model)
        .collect::<syn::Result<Vec<_>>>()?;

    let name = &input.ident;
    let idents: Vec<&Ident> = fields.iter().map(|field| &field.ident).collect();
    let names: Vec<&String> = fields.iter().map(|field| &field.name).collect();
    let bindings: Vec<_> = fields
        .iter()
        .enumerate()
        .map(|(at, field)| {
            let followed = fields[at + 1..]
                .iter()
                .any(|later| later.shape != Shape::Flag);

            bind(field, followed)
        })
        .collect();
    let described: Vec<_> = fields.iter().map(describe).collect();

    Ok(quote! {
        impl crate::command::Args for #name {
            const FIELDS: &'static [crate::command::args::Field] = &[#(#described),*];

            async fn parse(
                cx: &crate::command::cx::Cx,
                stream: &mut crate::command::stream::Stream,
            ) -> crate::command::error::Result<Self> {
                let (__flags, mut __owned) = crate::command::flags::split(
                    stream,
                    <Self as crate::command::Args>::FIELDS,
                );
                let __stream = &mut __owned;

                #(#bindings)*

                ::core::result::Result::Ok(Self { #(#idents),* })
            }

            fn snapshot(&self) -> ::serde_json::Value {
                let mut __out = ::serde_json::Map::new();

                #(
                    __out.insert(
                        ::std::string::String::from(#names),
                        crate::command::value::Snapshot::snapshot(&self.#idents),
                    );
                )*

                ::serde_json::Value::Object(__out)
            }
        }
    })
}

pub fn expand(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    if let Err(err) = syn::parse::<syn::parse::Nothing>(attr) {
        return err.to_compile_error().into();
    }

    let mut input: DeriveInput = match syn::parse(item) {
        Ok(parsed) => parsed,
        Err(err) => return err.to_compile_error().into(),
    };

    let generated = build(&input);

    if let Data::Struct(body) = &mut input.data {
        for field in body.fields.iter_mut() {
            field.attrs.retain(|attribute| {
                !attribute.path().is_ident("arg") && !attribute.path().is_ident("flag")
            });
        }
    }

    match generated {
        Ok(tokens) => quote! { #input #tokens }.into(),
        Err(err) => {
            let complaint = err.to_compile_error();

            quote! { #input #complaint }.into()
        }
    }
}
