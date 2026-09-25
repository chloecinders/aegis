use quote::{ToTokens, quote};
use syn::{Data, DeriveInput, Error, Fields, Ident, LitStr, Type};

use crate::command::model::peel;

struct Parameter {
    ident: Ident,
    name: String,
    inner: Type,
    required: bool,
    desc: LitStr,
}

fn model(field: &syn::Field) -> syn::Result<Parameter> {
    let ident = field
        .ident
        .clone()
        .ok_or_else(|| Error::new_spanned(field, "slash options must be named fields"))?;

    let mut declared = false;
    let mut name = None;
    let mut desc = None;

    for attribute in &field.attrs {
        if !attribute.path().is_ident("arg") {
            continue;
        }

        declared = true;

        attribute.parse_nested_meta(|option| {
            if option.path.is_ident("name") {
                name = Some(option.value()?.parse::<LitStr>()?.value());

                return Ok(());
            }

            if option.path.is_ident("desc") {
                desc = Some(option.value()?.parse::<LitStr>()?);

                return Ok(());
            }

            Err(option.error("a slash option only takes a name and a desc"))
        })?;
    }

    if !declared {
        return Err(Error::new_spanned(
            field,
            "slash options require #[arg(desc = \"...\")]",
        ));
    }

    let Some(desc) = desc else {
        return Err(Error::new_spanned(
            field,
            "slash options require desc = \"...\"",
        ));
    };

    if desc.value().is_empty() || desc.value().chars().count() > 100 {
        return Err(Error::new_spanned(
            &desc,
            "a slash option description is between 1 and 100 characters",
        ));
    }

    let name = name.unwrap_or_else(|| ident.to_string()).to_lowercase();

    if name.is_empty() || name.chars().count() > 32 {
        return Err(Error::new(
            ident.span(),
            "a slash option name is 1 to 32 characters",
        ));
    }

    let optional = peel(&field.ty, "Option");

    Ok(Parameter {
        ident,
        name,
        inner: optional.unwrap_or(&field.ty).clone(),
        required: optional.is_none(),
        desc,
    })
}

fn build(input: &DeriveInput) -> syn::Result<impl ToTokens + 'static> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "a slash command cannot be generic",
        ));
    }

    let Data::Struct(body) = &input.data else {
        return Err(Error::new_spanned(input, "#[slash] is an option struct"));
    };

    let parameters = match &body.fields {
        Fields::Named(declared) => declared
            .named
            .iter()
            .map(model)
            .collect::<syn::Result<Vec<_>>>()?,
        Fields::Unit => Vec::new(),
        Fields::Unnamed(unnamed) => {
            return Err(Error::new_spanned(
                unnamed,
                "slash options must be named fields",
            ));
        }
    };

    if let Some(misplaced) = parameters
        .iter()
        .skip_while(|parameter| parameter.required)
        .find(|parameter| parameter.required)
    {
        return Err(Error::new(
            misplaced.ident.span(),
            "required slash options come before optional ones",
        ));
    }

    let name = &input.ident;
    let idents: Vec<&Ident> = parameters
        .iter()
        .map(|parameter| &parameter.ident)
        .collect();
    let bindings = parameters.iter().map(|parameter| {
        let ident = &parameter.ident;
        let inner = &parameter.inner;
        let name = &parameter.name;

        match parameter.required {
            true => quote! {
                let #ident = crate::command::slash::value::required::<#inner>(cx, #name)?;
            },
            false => quote! {
                let #ident = crate::command::slash::value::optional::<#inner>(cx, #name)?;
            },
        }
    });
    let described = parameters.iter().map(|parameter| {
        let inner = &parameter.inner;
        let name = &parameter.name;
        let desc = &parameter.desc;
        let required = parameter.required;

        quote! {
            crate::command::slash::Parameter {
                name: #name,
                kind: <#inner as crate::command::slash::value::FromOption>::KIND,
                desc: #desc,
                required: #required,
            }
        }
    });

    Ok(quote! {
        impl crate::command::slash::Options for #name {
            const PARAMETERS: &'static [crate::command::slash::Parameter] = &[#(#described),*];

            fn parse(
                cx: &crate::command::slash::cx::SlashCx,
            ) -> crate::command::error::Result<Self> {
                #(#bindings)*

                ::core::result::Result::Ok(Self { #(#idents),* })
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
            field
                .attrs
                .retain(|attribute| !attribute.path().is_ident("arg"));
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
