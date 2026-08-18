use mendrel_syntax::generate::{Inventory, derive_inventory, render_inventory};
use mendrel_syntax::{SPACED_OPERATORS, TOKENS, TokenClass, production_rule};

#[test]
fn derives_sorted_grammar_inventory_from_the_normative_ebnf() {
    let grammar = include_str!("../../../spec/grammar.ebnf");
    let inventory = derive_inventory(grammar).expect("valid normative grammar");

    assert!(inventory.keywords.contains(&"module".to_owned()));
    assert!(inventory.keywords.contains(&"pub".to_owned()));
    assert!(inventory.keywords.contains(&"fn".to_owned()));
    assert!(inventory.punctuation.contains(&";".to_owned()));
    assert!(inventory.punctuation.contains(&"->".to_owned()));
    assert!(inventory.productions.contains(&"source_file".to_owned()));
    assert!(inventory.productions.contains(&"function_decl".to_owned()));
    assert!(
        inventory
            .productions
            .contains(&"additive_expression".to_owned())
    );
    assert!(inventory.keywords.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(
        inventory
            .punctuation
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    assert!(
        inventory
            .productions
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    assert_eq!(
        inventory
            .rules
            .iter()
            .find(|(name, _)| name == "module_decl")
            .map(|(_, rule)| rule.as_str()),
        Some("\"module\", qualified_name, \";\"")
    );
}

#[test]
fn rendered_inventory_is_independent_of_input_order() {
    let left = Inventory {
        keywords: vec!["fn".to_owned(), "module".to_owned()],
        punctuation: vec!["->".to_owned(), ";".to_owned()],
        productions: vec!["source_file".to_owned(), "function_decl".to_owned()],
        rules: vec![
            (
                "function_decl".to_owned(),
                "function_head, block".to_owned(),
            ),
            ("source_file".to_owned(), "function_decl, EOF".to_owned()),
        ],
    };
    let right = Inventory {
        keywords: vec!["module".to_owned(), "fn".to_owned()],
        punctuation: vec![";".to_owned(), "->".to_owned()],
        productions: vec!["function_decl".to_owned(), "source_file".to_owned()],
        rules: vec![
            ("source_file".to_owned(), "function_decl, EOF".to_owned()),
            (
                "function_decl".to_owned(),
                "function_head, block".to_owned(),
            ),
        ],
    };

    assert_eq!(render_inventory(&left), render_inventory(&right));
}

#[test]
fn generated_token_metadata_and_rules_are_queryable() {
    let function_keyword = TOKENS
        .iter()
        .find(|token| token.text == "fn")
        .expect("fn token metadata");
    let arrow = TOKENS
        .iter()
        .find(|token| token.text == "->")
        .expect("arrow token metadata");

    assert_eq!(function_keyword.class, TokenClass::Keyword);
    assert_eq!(arrow.class, TokenClass::Punctuation);
    assert_ne!(function_keyword.stable_id, arrow.stable_id);
    let mut token_ids = TOKENS
        .iter()
        .map(|token| token.stable_id)
        .collect::<Vec<_>>();
    token_ids.sort_unstable();
    token_ids.dedup();
    assert_eq!(token_ids.len(), TOKENS.len(), "stable token ID collision");
    assert!(SPACED_OPERATORS.binary_search(&"->").is_ok());
    assert_eq!(
        production_rule("module_decl"),
        Some("\"module\", qualified_name, \";\"")
    );
}

#[test]
fn malformed_ebnf_is_rejected_locally() {
    assert!(derive_inventory("source_file = \"module;").is_err());
    assert!(derive_inventory("(* never closes").is_err());
}
