extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(IntoVecU8)]
pub fn into_vec_u8_derive(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);

    // Used to get the name of the struct
    let name = input.ident;

    // The expanded code
    let expanded = quote! {
        impl Into<Vec<u8>> for #name {
            fn into(self) -> Vec<u8> {
                bincode::serialize(&self).expect("Should convert to VecU8 by bincode")
            }
        }
    };

    // Return the generated impl as a TokenStream
    TokenStream::from(expanded)
}

#[proc_macro_derive(TryFromSliceU8)]
pub fn from_slice_u8_derive(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);

    // Used to get the name of the struct
    let name = input.ident;

    // The expanded code
    let expanded = quote! {
        impl TryFrom<&[u8]> for #name {
            type Error = ();

            fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
                bincode::deserialize(value).map_err(|_| ())
            }
        }
    };

    // Return the generated impl as a TokenStream
    TokenStream::from(expanded)
}
