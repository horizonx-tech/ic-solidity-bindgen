use ethabi::param_type::ParamType;
use ethabi::{Function, StateMutability};
use inflector::cases::snakecase::to_snake_case;
use proc_macro2::{Ident, Span, TokenStream};
use quote::ToTokens as _;
use std::borrow::Borrow;
use std::path::Path;

fn ident<S: Borrow<str>>(name: S) -> Ident {
    Ident::new(name.borrow(), Span::call_site())
}

#[derive(Eq, PartialEq)]
enum Method {
    Send,
    Call,
}

fn method(f: &Function) -> Method {
    match f.state_mutability {
        StateMutability::Payable | StateMutability::NonPayable => Method::Send,
        StateMutability::Pure | StateMutability::View => Method::Call,
    }
}

pub fn abi_from_file(path: impl AsRef<Path>) -> TokenStream {
    let name = path
        .as_ref()
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let bytes = std::fs::read(path).unwrap();

    // See also 4cd1038f-56f2-4cf2-8dbe-672da9006083
    let abis = ethabi::Contract::load(&bytes[..]).expect("Could not validate ABIs");
    let abi_str = String::from_utf8(bytes).expect("Abis need to be valid UTF-8");

    let struct_name = ident(name);

    let mut send_fns = Vec::new();
    let mut call_fns = Vec::new();

    for f in abis.functions() {
        let dest = match method(f) {
            Method::Call => &mut call_fns,
            Method::Send => &mut send_fns,
        };

        let f = fn_from_abi(f);
        dest.push(f);
    }

    quote! {
        // "hygenic" ident for generic
        pub struct #struct_name<SolidityBindgenProvider> {
            provider: ::std::sync::Arc<SolidityBindgenProvider>,
            pub address: ::ic_web3::types::Address,
        }

        impl<SolidityBindgenProvider> ::std::clone::Clone for #struct_name<SolidityBindgenProvider> {
            fn clone(&self) -> Self {
                Self {
                    provider: ::std::clone::Clone::clone(&self.provider),
                    address: self.address,
                }
            }
        }

        impl<SolidityBindgenProvider> #struct_name<SolidityBindgenProvider> {
            pub fn new<Context>(address: ::ic_web3::types::Address, context: &Context) -> Self where Context: ::ic_solidity_bindgen::Context<Provider = SolidityBindgenProvider> {
                // Embed ABI into the program
                let abi = #abi_str;

                // Set up a wrapper so we can make calls
                let provider = ::ic_solidity_bindgen::Context::provider(context, address, abi.as_bytes());
                let provider = ::std::sync::Arc::new(provider);
                Self {
                    address,
                    provider,
                }
            }
        }

        impl<SolidityBindgenProvider> #struct_name<SolidityBindgenProvider> where SolidityBindgenProvider: ::ic_solidity_bindgen::SendProvider {

            // TODO: This API is not in the spirit of this library
            // (validating params & func at compile time). It may be better
            // to add options to all functions.
            pub async fn send(
                &self,
                func: &'static str,
                params: impl ic_web3::contract::tokens::Tokenize + Send,
                options: Option<::ic_web3::contract::Options>,
                confirmations: Option<usize>,
            ) -> Result<SolidityBindgenProvider::Out, ::ic_web3::Error> {
                self.provider.send(func, params, options, confirmations).await
            }

            #(#send_fns)*
        }

        impl<SolidityBindgenProvider> #struct_name<SolidityBindgenProvider>
            where SolidityBindgenProvider: ::ic_solidity_bindgen::CallProvider {
                #(#call_fns)*
        }
    }
}

/// Convert some Ethereum ABI type to a Rust type (usually from the web3 namespace)
/// Returns the tokens for the type, as well as the level of nesting of the tuples for a hack.
fn param_type(kind: &ParamType) -> (TokenStream, usize) {
    match kind {
        ParamType::Address => (quote! { ::ic_web3::types::Address }, 0),
        ParamType::Bytes => (quote! { ::std::vec::Vec<u8> }, 0),
        ParamType::Int(size) => match size {
            256 => (quote! { ::ic_solidity_bindgen::internal::Unimplemented }, 0),
            _ => (ident(format!("i{}", size)).to_token_stream(), 0),
        },
        ParamType::Uint(size) => match size {
            256 => (quote! { ::ic_web3::types::U256 }, 0),
            _ => {
                let name = ident(format!("u{}", size));
                (quote! { #name }, 0)
            }
        },
        ParamType::Bool => (quote! { bool }, 0),
        ParamType::String => (quote! { ::std::string::String }, 0),
        ParamType::Array(inner) => {
            let (inner, nesting) = param_type(inner);
            if nesting > 0 {
                (quote! { ::ic_solidity_bindgen::internal::Unimplemented }, 0)
            } else {
                (quote! { ::std::vec::Vec<#inner> }, nesting)
            }
        }
        ParamType::FixedBytes(len) => (quote! { [ u8; #len ] }, 0),
        ParamType::FixedArray(inner, len) => {
            let (inner, nesting) = param_type(inner);
            (quote! { [#inner; #len] }, nesting)
        }
        ParamType::Tuple(members) => match members.len() {
            0 => (quote! { ::ic_solidity_bindgen::internal::Empty }, 1),
            _ => {
                let members: Vec<_> = members
                    .into_iter()
                    .map(|member| param_type(member))
                    .collect();
                // Unwrap is ok because in this branch there must be at least 1 item.
                let nesting = 1 + members.iter().map(|(_, n)| *n).max().unwrap();
                let types = members.iter().map(|(ty, _)| ty);
                (quote! { (#(#types,)*) }, nesting)
            }
        },
    }
}

pub fn to_rust_name(type_name: &str, eth_name: &str, i: usize) -> String {
    if eth_name == "" {
        format!("{}_{}", type_name, i)
    } else {
        to_snake_case(eth_name)
    }
}

pub fn fn_from_abi(function: &Function) -> TokenStream {
    let eth_name = &function.name;
    let rust_name = ident(to_rust_name("function", eth_name, 0));

    // Get the types and names of parameters
    let params_nesting = if function.inputs.len() > 1 { 1 } else { 0 };
    let params_in = function.inputs.iter().enumerate().map(|(i, param)| {
        let name = ident(to_rust_name("input", &param.name, i));
        let (t, nesting) = param_type(&param.kind);

        // We have to have a branch here because Tokenize isn't implemented for
        // nested tuples. This is because the impls of Tokenize for (A, B, ..)
        // require the members to implement Tokenizable instead of Tokenize.
        // Even if this did compile, it doesn't seem ethabi is architected in a
        // way to deal with this properly considering the separation between
        // dynamic and static types, and there are some issues like this one:
        // https://github.com/openethereum/ethabi/issues/178
        // Changing this type to Unimplemented always reduces the amount of
        // nesting to 1 or 0 which compiles.
        if nesting + params_nesting > 1 {
            quote! {
                #name: ::ic_solidity_bindgen::internal::Unimplemented
            }
        } else {
            quote! {
                #name: #t
            }
        }
    });

    let params = function
        .inputs
        .iter()
        .enumerate()
        .map(|(i, param)| ident(to_rust_name("input", &param.name, i)).into_token_stream());
    let params = if function.inputs.len() == 1 {
        quote! { #(#params)* }
    } else {
        quote! { (#(#params),*) }
    };

    let method = method(function);

    let ok = if method == Method::Send {
        // Despite information in the ABIs to the contrary, there aren't
        // really outputs for web3 send fns. The outputs that are
        // available aren't returned by these APIs, but are only made
        // available to contracts calling each other. 🤷
        //
        // All you can get is a receipt. So, the way to get something
        // like a return value would be to check for events emitted or
        // to make further queries for data.
        quote! { SolidityBindgenProvider::Out }
    } else {
        match function.outputs.len() {
            0 => quote! { ::ic_solidity_bindgen::internal::Empty },
            1 => {
                let (t, nesting) = param_type(&function.outputs[0].kind);
                if nesting < 2 {
                    t
                } else {
                    quote! {
                        ::ic_solidity_bindgen::internal::Unimplemented
                    }
                }
            }
            _ => {
                let types = function.outputs.iter().map(|o| {
                    let (t, nesting) = param_type(&o.kind);
                    if nesting != 0 {
                        quote! {
                            ::ic_solidity_bindgen::internal::Unimplemented
                        }
                    } else {
                        t
                    }
                });

                quote! { (#(#types),*) }
            }
        }
    };

    let fn_call = match method {
        Method::Call => quote! { self.provider.call(#eth_name, #params).await },
        Method::Send => quote! { self.provider.send(#eth_name, #params, None, None).await },
    };

    quote! {
        pub async fn #rust_name(&self, #(#params_in),*) -> ::std::result::Result<#ok, ::ic_web3::Error> {
            #fn_call
        }
    }
}
