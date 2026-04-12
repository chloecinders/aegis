use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, FnArg, Generics, ItemFn, Pat, PatIdent, PatType, WhereClause, parse_macro_input,
};

#[proc_macro_attribute]
pub fn command(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;

    let mut transformers = Vec::new();
    let mut arg_bindings = Vec::new();
    let mut main_bindings = Vec::new();
    let mut needs_args_iter = false;

    // First pass to see if we need args iter
    for arg in &sig.inputs {
        if let FnArg::Typed(PatType { attrs, .. }) = arg {
            for attr in attrs {
                if parse_transformer_attr(attr.clone()).is_some() {
                    needs_args_iter = true;
                }
            }
        }
    }

    for arg in &sig.inputs {
        let FnArg::Typed(PatType { attrs, pat, ty, .. }) = arg else {
            continue;
        };

        let Pat::Ident(PatIdent { ident: binding, mutability, .. }) = &**pat else {
            panic!("Invalid pattern binding");
        };

        let mut is_transformer = false;
        let binding_arg = format_ident!("_{}_arg", binding, span = binding.span());

        for attr in attrs {
            let Some(transformer) = parse_transformer_attr(attr.clone()) else {
                continue;
            };
            is_transformer = true;
            transformers.push(transformer);

            let (variant_ident, is_option) = match &**ty {
                syn::Type::Path(type_path) => {
                    let seg = type_path.path.segments.last().unwrap();
                    match (seg.ident == "Option", &seg.arguments) {
                        (true, syn::PathArguments::AngleBracketed(args)) => {
                            let syn::GenericArgument::Type(syn::Type::Path(inner)) =
                                args.args.first().unwrap()
                            else { panic!("Unsupported Option inner type"); };
                            (format_ident!("{}", inner.path.segments.last().unwrap().ident), true)
                        }
                        (false, _) => (format_ident!("{}", seg.ident), false),
                        _ => panic!("Unsupported Option type"),
                    }
                }
                _ => panic!("Unsupported argument type"),
            };

            let binding_str = binding.to_string();
            let variant_str = variant_ident.to_string();

            let binding_exp = match is_option {
                true => {
                    let syn::Type::Path(type_path) = &**ty else { unreachable!() };
                    let inner_ty = match &type_path.path.segments.last().unwrap().arguments {
                        syn::PathArguments::AngleBracketed(args) => args.args.first().unwrap(),
                        _ => unreachable!(),
                    };
                    quote! {
                        let (#binding_arg, #mutability #binding): (Option<Token>, Option<#inner_ty>) = {
                            let tok = args_iter.next();
                            match tok.clone() {
                                Some(Token { contents: Some(CommandArgument::#variant_ident(inner_v)), .. }) => (tok.clone(), Some(inner_v)),
                                Some(tok) => (Some(tok), None),
                                None => (None, None),
                            }
                        };
                    }
                }
                false => quote! {
                    let #binding_arg = match args_iter.next() {
                        Some(tok @ Token { contents: Some(CommandArgument::#variant_ident(_)), .. }) => tok,
                        _ => return Box::pin(async move { Err(CommandError::arg_not_found(#binding_str, Some(#variant_str))) }),
                    };
                    let #mutability #binding = match &#binding_arg {
                        Token { contents: Some(CommandArgument::#variant_ident(v)), .. } => v.clone(),
                        _ => unreachable!(),
                    };
                },
            };
            arg_bindings.push(binding_exp);
        }

        if !is_transformer {
            let binding_name = binding.to_string();
            let clean_name = binding_name.trim_start_matches('_');
            match clean_name {
                "ctx" => main_bindings.push(quote! { let #mutability #binding = __ctx; }),
                "msg" => main_bindings.push(quote! { let #mutability #binding = __msg; }),
                "handler" => main_bindings.push(quote! { let #mutability #binding = __handler; }),
                "args" => {
                    if needs_args_iter {
                        main_bindings.push(quote! { let #mutability #binding = __args.clone(); });
                    } else {
                        main_bindings.push(quote! { let #mutability #binding = __args; });
                    }
                },
                "params" => main_bindings.push(quote! { let #mutability #binding = __params; }),
                "trace" => main_bindings.push(quote! { let #mutability #binding = __trace; }),
                _ => panic!("Unknown dependency requested: {}", binding_name),
            }
        }
    }

    let transformer_fns = transformers.iter().map(|t| {
        let ident = format_ident!("{}", t);
        quote! { Arc::new(Transformers::#ident) }
    });

    let fn_name = &sig.ident;
    let fn_async = &sig.asyncness;
    let fn_output = &sig.output;

    let fn_generics: Generics = syn::parse_quote! {<'life0, 'life1, 'life2, 'life3, 'async_trait>};
    let fn_where: WhereClause = syn::parse_quote! {
        where
            'life0: 'async_trait,
            'life1: 'async_trait,
            'life2: 'async_trait,
            'life3: 'async_trait,
            Self: 'async_trait
    };

    let stmts = &block.stmts;

    let args_iter_stmt = if needs_args_iter {
        quote! { let mut args_iter = __args.into_iter(); }
    } else {
        quote! {}
    };

    TokenStream::from(quote! {
        #vis #fn_async fn #fn_name #fn_generics (
            &'life0 self,
            __ctx: Context,
            __msg: Message,
            __handler: &'life1 Handler,
            __args: Vec<Token>,
            __params: std::collections::HashMap<&'life2 str, (bool, CommandArgument)>,
            __trace: &'life3 mut crate::utils::trace::TraceContext,
        ) #fn_output #fn_where {
            #(#main_bindings)*
            #args_iter_stmt
            #(#arg_bindings)*
            #(#stmts)*
        }

        fn get_transformers(&self) -> Vec<TransformerFnArc> {
            vec![#(#transformer_fns),*]
        }
    })
}

fn parse_transformer_attr(attr: Attribute) -> Option<String> {
    let mut it = attr.meta.path().segments.iter();
    matches!(it.next()?.ident.to_string().as_str(), "transformers")
        .then(|| it.next().map(|s| s.ident.to_string()))
        .flatten()
}
