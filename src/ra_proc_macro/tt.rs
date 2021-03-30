use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

// https://github.com/rust-analyzer/rust-analyzer/blob/2021-03-29/crates/proc_macro_api/src/rpc.rs

#[derive(Clone, Deserialize, Serialize)]
enum TokenTree {
    Leaf(Leaf),
    Subtree(Subtree),
}

impl From<proc_macro2::TokenTree> for TokenTree {
    fn from(tt: proc_macro2::TokenTree) -> Self {
        match tt {
            proc_macro2::TokenTree::Group(group) => TokenTree::Subtree(group.into()),
            proc_macro2::TokenTree::Ident(ident) => TokenTree::Leaf(Leaf::Ident(ident.into())),
            proc_macro2::TokenTree::Punct(punct) => TokenTree::Leaf(Leaf::Punct(punct.into())),
            proc_macro2::TokenTree::Literal(lit) => TokenTree::Leaf(Leaf::Literal(lit.into())),
        }
    }
}

impl From<TokenTree> for proc_macro2::TokenTree {
    fn from(tt: TokenTree) -> Self {
        match tt {
            TokenTree::Subtree(group) => proc_macro2::TokenTree::Group(group.into()),
            TokenTree::Leaf(Leaf::Ident(ident)) => proc_macro2::TokenTree::Ident(ident.into()),
            TokenTree::Leaf(Leaf::Punct(punct)) => proc_macro2::TokenTree::Punct(punct.into()),
            TokenTree::Leaf(Leaf::Literal(lit)) => proc_macro2::TokenTree::Literal(lit.into()),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Subtree {
    delimiter: Option<Delimiter>,
    token_trees: Vec<TokenTree>,
}

impl From<proc_macro2::Group> for Subtree {
    fn from(group: proc_macro2::Group) -> Self {
        return Subtree {
            delimiter: match group.delimiter() {
                proc_macro2::Delimiter::Parenthesis => Some(delimiter(DelimiterKind::Parenthesis)),
                proc_macro2::Delimiter::Brace => Some(delimiter(DelimiterKind::Brace)),
                proc_macro2::Delimiter::Bracket => Some(delimiter(DelimiterKind::Bracket)),
                proc_macro2::Delimiter::None => None,
            },
            token_trees: group.stream().into_iter().map(Into::into).collect(),
        };

        fn delimiter(kind: DelimiterKind) -> Delimiter {
            Delimiter { kind }
        }
    }
}

impl From<Subtree> for proc_macro2::Group {
    fn from(subtree: Subtree) -> Self {
        let delimiter = match subtree.delimiter.map(|Delimiter { kind, .. }| kind) {
            Some(DelimiterKind::Parenthesis) => proc_macro2::Delimiter::Parenthesis,
            Some(DelimiterKind::Brace) => proc_macro2::Delimiter::Brace,
            Some(DelimiterKind::Bracket) => proc_macro2::Delimiter::Bracket,
            None => proc_macro2::Delimiter::None,
        };
        let token_stream = subtree
            .token_trees
            .iter()
            .cloned()
            .map(proc_macro2::TokenTree::from)
            .collect();
        proc_macro2::Group::new(delimiter, token_stream)
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
struct Delimiter {
    kind: DelimiterKind,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum DelimiterKind {
    Parenthesis,
    Brace,
    Bracket,
}

#[derive(Clone, Deserialize, Serialize)]
enum Leaf {
    Literal(Literal),
    Punct(Punct),
    Ident(Ident),
}

#[derive(Clone, Deserialize, Serialize)]
struct Literal {
    text: SmolStr,
}

impl From<proc_macro2::Literal> for Literal {
    fn from(lit: proc_macro2::Literal) -> Self {
        Self {
            text: lit.to_string().into(),
        }
    }
}

impl From<Literal> for proc_macro2::Literal {
    fn from(lit: Literal) -> Self {
        syn::parse_str(&lit.text)
            .unwrap_or_else(|e| panic!("could not parse {:?} as a literal: {}", &lit.text, e))
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
struct Punct {
    char: char,
    spacing: Spacing,
}

impl From<proc_macro2::Punct> for Punct {
    fn from(punct: proc_macro2::Punct) -> Self {
        Self {
            char: punct.as_char(),
            spacing: punct.spacing().into(),
        }
    }
}

impl From<Punct> for proc_macro2::Punct {
    fn from(punct: Punct) -> Self {
        proc_macro2::Punct::new(punct.char, punct.spacing.into())
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
enum Spacing {
    Alone,
    Joint,
}

impl From<proc_macro2::Spacing> for Spacing {
    fn from(spacing: proc_macro2::Spacing) -> Self {
        match spacing {
            proc_macro2::Spacing::Alone => Spacing::Alone,
            proc_macro2::Spacing::Joint => Spacing::Joint,
        }
    }
}

impl From<Spacing> for proc_macro2::Spacing {
    fn from(spacing: Spacing) -> Self {
        match spacing {
            Spacing::Alone => proc_macro2::Spacing::Alone,
            Spacing::Joint => proc_macro2::Spacing::Joint,
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct Ident {
    text: SmolStr,
}

impl From<proc_macro2::Ident> for Ident {
    fn from(ident: proc_macro2::Ident) -> Self {
        Self {
            text: ident.to_string().into(),
        }
    }
}

impl From<Ident> for proc_macro2::Ident {
    fn from(ident: Ident) -> Self {
        proc_macro2::Ident::new(&ident.text, proc_macro2::Span::call_site())
    }
}
