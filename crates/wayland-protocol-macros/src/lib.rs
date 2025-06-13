use anyhow::{Context, Result};
use proc_macro::TokenStream;
use quick_xml::de::from_str;
use quote::quote;
use schema::{Interface, Protocol};
use std::{fs, path::Path};
use syn::{parse_macro_input, LitStr};

mod schema;

fn parse_wayland_xml(xml_path: &str) -> Result<Protocol> {
    let xml_content = fs::read_to_string(xml_path)
        .with_context(|| format!("Failed to read XML file: {}", xml_path))?;

    let protocol: Protocol =
        from_str(&xml_content).map_err(|e| anyhow::anyhow!("Failed to parse XML file: {}", e))?;

    Ok(protocol)
}

fn rust_type_from_wayland_type(
    wayland_type: &str,
    interface: Option<&str>,
    allow_null: bool,
) -> proc_macro2::TokenStream {
    let base_type = match wayland_type {
        "int" => quote! { i32 },
        "uint" => quote! { u32 },
        "fixed" => quote! { i32 }, // Wayland fixed-point number
        "string" => quote! { String },
        "object" => {
            if let Some(iface) = interface {
                let iface_ident = syn::Ident::new(
                    &format!("{}Object", snake_to_pascal_case(iface)),
                    proc_macro2::Span::call_site(),
                );
                quote! { #iface_ident }
            } else {
                quote! { ObjectId }
            }
        }
        "new_id" => {
            if let Some(iface) = interface {
                let iface_ident = syn::Ident::new(
                    &format!("{}Object", snake_to_pascal_case(iface)),
                    proc_macro2::Span::call_site(),
                );
                quote! { #iface_ident }
            } else {
                quote! { ObjectId }
            }
        }
        "array" => quote! { Vec<u8> },
        "fd" => quote! { std::os::unix::io::RawFd },
        _ => quote! { () }, // Unknown type
    };

    if allow_null {
        quote! { Option<#base_type> }
    } else {
        base_type
    }
}

fn snake_to_pascal_case(s: &str) -> String {
    s.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect()
}

fn generate_interface_code(interface: Interface) -> proc_macro2::TokenStream {
    let interface_name = syn::Ident::new(
        &snake_to_pascal_case(&interface.name),
        proc_macro2::Span::call_site(),
    );

    // Generate enums
    let enums = interface
        .interface_enum
        .unwrap_or_default()
        .into_iter()
        .map(|enum_def| {
            let enum_name = syn::Ident::new(
                &format!("{}{}", interface_name, snake_to_pascal_case(&enum_def.name)),
                proc_macro2::Span::call_site(),
            );
            let entries = enum_def.entry.iter().map(|entry| {
                let entry_name =
                    syn::Ident::new(&entry.name.to_uppercase(), proc_macro2::Span::call_site());
                let value = entry.value.parse::<u32>().unwrap_or(0);
                quote! { #entry_name = #value }
            });

            quote! {
                #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                #[repr(u32)]
                pub enum #enum_name {
                    #(#entries,)*
                }
            }
        });

    // Generate request structs
    let requests = interface
        .request
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(opcode, request)| {
            let request_name = syn::Ident::new(
                &format!(
                    "{}{}Request",
                    interface_name,
                    snake_to_pascal_case(&request.name)
                ),
                proc_macro2::Span::call_site(),
            );
            let opcode_lit = opcode as u16;

            let fields = request.arg.unwrap_or_default().into_iter().map(|arg| {
                let field_name = syn::Ident::new(&arg.name, proc_macro2::Span::call_site());
                let field_type = rust_type_from_wayland_type(
                    &arg.arg_type,
                    arg.interface.as_deref(),
                    arg.allow_null.unwrap_or(false),
                );
                quote! { pub #field_name: #field_type }
            });

            quote! {
                #[derive(Debug, Clone)]
                pub struct #request_name {
                    #(#fields,)*
                }

                impl #request_name {
                    pub const OPCODE: u16 = #opcode_lit;
                }
            }
        });

    // Generate event structs
    let events = interface
        .event
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(opcode, event)| {
            let event_name = syn::Ident::new(
                &format!(
                    "{}{}Event",
                    interface_name,
                    snake_to_pascal_case(&event.name)
                ),
                proc_macro2::Span::call_site(),
            );
            let opcode_lit = opcode as u16;

            let fields = event.arg.unwrap_or_default().into_iter().map(|arg| {
                let field_name = syn::Ident::new(&arg.name, proc_macro2::Span::call_site());
                let field_type = rust_type_from_wayland_type(
                    &arg.arg_type,
                    arg.interface.as_deref(),
                    arg.allow_null.unwrap_or(false),
                );
                quote! { pub #field_name: #field_type }
            });

            quote! {
                #[derive(Debug, Clone)]
                pub struct #event_name {
                    #(#fields,)*
                }

                impl #event_name {
                    pub const OPCODE: u16 = #opcode_lit;
                }
            }
        });

    quote! {
        // Enums
        #(#enums)*

        // Requests
        #(#requests)*

        // Events
        #(#events)*
    }
}

/// Generate Wayland protocol structs from an XML file
///
/// # Example
/// ```rust
/// use wayland_protocol_macros::wayland_protocol;
///
/// wayland_protocol!("path/to/wayland.xml");
/// ```
#[proc_macro]
pub fn wayland_protocol(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as LitStr);
    let xml_path = input.value();

    // Make path relative to CARGO_MANIFEST_DIR if it's not absolute
    let xml_path = if Path::new(&xml_path).is_absolute() {
        xml_path
    } else {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        format!("{}/{}", manifest_dir, xml_path)
    };

    let protocol = match parse_wayland_xml(&xml_path) {
        Ok(protocol) => protocol,
        Err(e) => {
            return syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("Failed to parse Wayland XML: {}", e),
            )
            .to_compile_error()
            .into();
        }
    };

    let interfaces = protocol.interface.into_iter().map(generate_interface_code);

    let expanded = quote! {
        // Common types
        pub type ObjectId = u32;

        #(#interfaces)*
    };

    TokenStream::from(expanded)
}
