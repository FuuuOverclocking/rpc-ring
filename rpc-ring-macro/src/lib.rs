use quote::quote;
use syn::Token;
use syn::parse::Parse;

struct Schema {
    sqe: SchemaSqe,
    cqe: SchemaCqe,
    entries: Vec<SchemaEntry>,
}

impl Parse for Schema {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let sqe = input.parse()?;
        let cqe = input.parse()?;

        let mut entries = Vec::new();
        while !input.is_empty() {
            entries.push(input.parse()?);
        }

        Ok(Self { sqe, cqe, entries })
    }
}

struct SchemaSqe {
    name: syn::Ident,
    size: syn::LitInt,
    req: syn::Ident,
}

impl Parse for SchemaSqe {
    // e.g. `struct Sqe: size = 64, enum Request;`
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.parse::<Token![struct]>()?;
        let name = input.parse()?;

        input.parse::<Token![:]>()?;

        let ident = input.parse::<syn::Ident>()?;
        if ident != "size" {
            return Err(syn::Error::new_spanned(ident, "expected identifier `size`"));
        }
        input.parse::<Token![=]>()?;
        let size = input.parse()?;

        input.parse::<Token![,]>()?;

        input.parse::<Token![enum]>()?;
        let req = input.parse()?;

        input.parse::<Token![;]>()?;

        Ok(Self { name, size, req })
    }
}

struct SchemaCqe {
    name: syn::Ident,
    size: syn::LitInt,
    resp: syn::Ident,
}

impl Parse for SchemaCqe {
    // e.g. `struct Cqe: size = 64, union Response;`
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        input.parse::<Token![struct]>()?;
        let name = input.parse()?;

        input.parse::<Token![:]>()?;

        let ident = input.parse::<syn::Ident>()?;
        if ident != "size" {
            return Err(syn::Error::new_spanned(ident, "expected identifier `size`"));
        }
        input.parse::<Token![=]>()?;
        let size = input.parse()?;

        input.parse::<Token![,]>()?;

        input.parse::<Token![union]>()?;
        let resp = input.parse()?;

        input.parse::<Token![;]>()?;

        Ok(Self { name, size, resp })
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

    let sqe_name = schema.sqe.name;
    let sqe_size = schema.sqe.size;
    let req_name = schema.sqe.req;

    let cqe_name = schema.cqe.name;
    let cqe_size = schema.cqe.size;
    let resp_name = schema.cqe.resp;

    let output = quote! {
        const _: () = assert!(size_of::<#sqe_name>() == #sqe_size);
        #[repr(C, align(#sqe_size))]
        struct #sqe_name {
            id: u64,
            req: #req_name,
        }

        #[repr(C, u32)]
        #[non_exhaustive]
        pub enum #req_name {
            #(
                #request_types (::core::mem::ManuallyDrop< #request_types > ) #request_discriminant_tokens
            ),*
        }

        impl #req_name {
            fn op(&self) -> u32 {
                // If the enumeration specifies a primitive representation, then
                // the discriminant may be reliably accessed via unsafe pointer
                // casting.
                // Ref: https://doc.rust-lang.org/reference/items/enumerations.html#r-items.enum.discriminant.access-memory
                unsafe { *(self as *const Self as *const u32) }
            }
        }

        const _: () = assert!(size_of::<#cqe_name>() == #cqe_size);
        #[repr(C, align(#cqe_size))]
        struct #cqe_name {
            id: u64,
            resp: #resp_name,
        }

        #[repr(C)]
        pub union #resp_name {
            #(
                #response_field_names: ::core::mem::ManuallyDrop<#response_types>
            ),*
        }

        pub fn dispatch(request: #req_name) {
            match request {
                #( #match_arms )*
            }
        }
    };

    proc_macro::TokenStream::from(output)
}
