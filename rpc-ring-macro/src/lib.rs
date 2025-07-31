use quote::quote;
use syn::Token;
use syn::parse::Parse;

struct Schema {
    entries: Vec<SchemaEntry>,
}

impl Parse for Schema {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut entries = Vec::new();
        while !input.is_empty() {
            entries.push(input.parse()?);
        }

        Ok(Self { entries })
    }
}

struct SchemaEntry {
    discriminant: Option<syn::LitInt>,
    req: syn::Type,
    resp: syn::Type,
}

impl Parse for SchemaEntry {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let discriminant = if input.peek(syn::LitInt) && input.peek2(Token![:]) {
            let lit: syn::LitInt = input.parse()?;
            input.parse::<Token![:]>()?;
            Some(lit)
        } else {
            None
        };

        let req: syn::Type = input.parse()?;
        input.parse::<Token![->]>()?;
        let resp: syn::Type = input.parse()?;

        input.parse::<Token![;]>()?;

        Ok(Self {
            discriminant,
            req,
            resp,
        })
    }
}
#[proc_macro]
pub fn def_schema(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let schema = match syn::parse::<Schema>(input) {
        Ok(s) => s,
        Err(e) => return e.to_compile_error().into(),
    };

    let request_types = schema.entries.iter().map(|entry| &entry.req);
    let request_discriminant_tokens = schema
        .entries
        .iter()
        .map(|entry| entry.discriminant.as_ref().map(|d| quote! { = #d }));

    let response_types = schema.entries.iter().map(|entry| &entry.resp);
    let response_field_names_result: syn::Result<Vec<syn::Ident>> = schema
        .entries
        .iter()
        .map(|entry| {
            let type_path = if let syn::Type::Path(type_path) = &entry.req {
                type_path
            } else {
                return Err(syn::Error::new_spanned(
                    &entry.req,
                    "Request type must be a path-like type (e.g., 'MyRequest')",
                ));
            };

            let req_type_ident = if let Some(ident) = type_path.path.get_ident() {
                ident
            } else {
                return Err(syn::Error::new_spanned(
                    &type_path.path,
                    "Request type must be a single identifier (e.g., 'MyRequest'), not a path (e.g., 'std::io::Error')",
                ));
            };

            let name_str = heck::AsSnakeCase(req_type_ident.to_string()).to_string();
            Ok(syn::Ident::new(&name_str, req_type_ident.span()))
        })
        .collect();
    let response_field_names = match response_field_names_result {
        Ok(names) => names,
        Err(e) => return e.to_compile_error().into(),
    };

    let match_arms = schema.entries.iter().map(|entry| {
        let req_type = &entry.req;
        let resp_type = &entry.resp;
        quote! {
            Request::#req_type(payload) => {
                println!(
                    "Dispatching request: '{}', expects response: '{}'",
                    stringify!(#req_type),
                    stringify!(#resp_type)
                );
            }
        }
    });

    let output = quote! {
        #[repr(C, u32)]
        #[non_exhaustive]
        pub enum Request {
            #(
                #request_types ( #request_types ) #request_discriminant_tokens
            ),*
        }

        impl Request {
            fn discriminant(&self) -> u32 {
                // If the enumeration specifies a primitive representation, then
                // the discriminant may be reliably accessed via unsafe pointer
                // casting.
                // Ref: https://doc.rust-lang.org/reference/items/enumerations.html#r-items.enum.discriminant.access-memory
                unsafe { *(self as *const Self as *const u32) }
            }
        }

        #[repr(C)]
        pub union Response {
            #(
                #response_field_names: ::core::mem::ManuallyDrop<#response_types>
            ),*
        }

        pub fn dispatch(request: Request) {
            match request {
                #( #match_arms )*
            }
        }
    };

    proc_macro::TokenStream::from(output)
}
