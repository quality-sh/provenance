//! Marker attributes binding code to provenance rules, and the derive that
//! declares a record kind's reference fields.

mod relations;

use proc_macro::{TokenStream, TokenTree};

/// Marks a function or type as a Provenance Rule's primary implementation.
///
/// Takes the rule's id as a single string literal:
///
/// ```ignore
/// #[rule("rule_prov_edge_endpoint_table")]
/// pub fn validate_edge_endpoint(...) { ... }
/// ```
///
/// The attribute binds the item to an independent Rule record. It does
/// not change the item it is attached to. Because it is a compiled symbol
/// rather than a comment, it moves with refactors, dies with deleted code,
/// and the scanner can check the cited Rule id against the graph.
#[proc_macro_attribute]
pub fn rule(attr: TokenStream, item: TokenStream) -> TokenStream {
    validate_rule_id(&attr);
    item
}

const VERIFICATION_METHODS: [&str; 6] = [
    "exhaustion",
    "property",
    "examples",
    "conformance",
    "construction",
    "proof",
];

/// Marks an item as verifying a provenance rule, and says how.
///
/// Takes the rule's id and one method word:
///
/// ```ignore
/// #[test]
/// #[verifies("rule_prov_edge_endpoint_table", exhaustion)]
/// fn endpoint_table_conforms_to_rule_leaf() { ... }
/// ```
///
/// Methods: `exhaustion` (every input in a finite domain is tried),
/// `property` (generated inputs checked against a stated property),
/// `examples` (hand-picked cases), `conformance` (an independent expression
/// of the Rule is checked against its primary implementation), `construction`
/// (a type or constraint makes violation impossible; goes on the type, not a test),
/// `proof` (a machine-checked proof outside this test runner backs the rule;
/// the marked site is the bridge that pins the implementation to the proved
/// model, such as a golden-vector test shared with a Lean theorem).
///
/// A rule with no `#[verifies]` site anywhere is unverified; that state is
/// reported by the scanner, never written.
#[proc_macro_attribute]
pub fn verifies(attr: TokenStream, item: TokenStream) -> TokenStream {
    validate_verifies_args(&attr);
    item
}

fn validate_verifies_args(attr: &TokenStream) {
    let tokens: Vec<TokenTree> = attr.clone().into_iter().collect();
    match tokens.as_slice() {
        [TokenTree::Literal(literal), TokenTree::Punct(punct), TokenTree::Ident(method)]
            if punct.as_char() == ',' =>
        {
            let repr = literal.to_string();
            assert!(
                repr.len() > 2 && repr.starts_with('"') && repr.ends_with('"'),
                "verifies takes the rule id as a non-empty string literal, got {repr}"
            );
            let method = method.to_string();
            assert!(
                VERIFICATION_METHODS.contains(&method.as_str()),
                "unknown verification method `{method}`; expected one of: {}",
                VERIFICATION_METHODS.join(", ")
            );
        }
        _ => panic!("verifies takes a rule id string literal and a method word"),
    }
}

fn validate_rule_id(attr: &TokenStream) {
    let tokens: Vec<TokenTree> = attr.clone().into_iter().collect();
    match tokens.as_slice() {
        [TokenTree::Literal(literal)] => {
            let repr = literal.to_string();
            assert!(
                repr.len() > 2 && repr.starts_with('"') && repr.ends_with('"'),
                "rule takes its id as a non-empty string literal, got {repr}"
            );
        }
        _ => panic!("rule takes exactly one rule id string literal"),
    }
}

/// Declares the reference fields of one record kind.
///
/// Every `StableId`-typed field carries `#[relation(target = Kind, flow =
/// target_upstream | target_downstream | none [, required] [, name = "..."]
/// [, via = field])]` or `#[relation(none)]`; the field named `id` is the
/// owner key. The derive emits `Kind::RELATIONS` and `impl RelationOwner`.
#[proc_macro_derive(Relations, attributes(relation))]
pub fn relations(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    relations::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
