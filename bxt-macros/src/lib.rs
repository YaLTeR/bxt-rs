//! Attribute macro for creating a byte pattern out of a more commonly used format.

use std::fmt::Write;

extern crate proc_macro;
use proc_macro::{Span, TokenStream, TokenTree};

/// Converts the item to a byte pattern.
#[proc_macro]
pub fn pattern(input: TokenStream) -> TokenStream {
    let mut output = String::from("&[");

    let mut question = None;
    let mut last_token = None;
    for token in input {
        last_token = Some(token.clone());

        match token {
            TokenTree::Punct(punct) if punct.as_char() == '?' => {
                if question.is_some() {
                    question = None;
                    output.push_str("None, ");
                } else {
                    question = Some(punct);
                }
            }
            token => {
                if let Some(punct) = question {
                    return error(punct.span(), "missing second `?`");
                }

                let token_string = token.to_string();
                if token_string.len() != 2 {
                    return error(token.span(), "token must be 2 characters long");
                }

                match u8::from_str_radix(&token_string, 16) {
                    Ok(byte) => write!(output, "Some(0x{:X}), ", byte)
                        .expect("writing to `String` should never error"),
                    Err(_) => return error(token.span(), "token must be a hex number"),
                }
            }
        }
    }

    if let Some(punct) = question {
        return error(punct.span(), "missing second `?`");
    }

    if let Some(TokenTree::Punct(punct)) = last_token {
        return error(
            punct.span(),
            "pattern ends on `??` (probably not what you want)",
        );
    }

    output.push(']');
    output.parse().unwrap()
}

fn error(span: Span, msg: &str) -> TokenStream {
    format!(r#"compile_error!("{}")"#, msg)
        .parse::<TokenStream>()
        .unwrap()
        .into_iter()
        .map(|mut t| {
            t.set_span(span);
            t
        })
        .collect()
}

#[test]
fn ui() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
