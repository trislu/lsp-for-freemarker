// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

//! This crate provides Freemarker language support for the [tree-sitter][] parsing library.
//!
//! Typically, you will use the [LANGUAGE][] constant to add this language to a
//! tree-sitter [Parser][], and then use the parser to parse some code:
//!
//! ```
//! let code = r#"
//! "#;
//! let mut parser = tree_sitter::Parser::new();
//! let language = tree_sitter_freemarker::LANGUAGE;
//! parser
//!     .set_language(&language.into())
//!     .expect("Error loading Freemarker parser");
//! let tree = parser.parse(code, None).unwrap();
//! assert!(!tree.root_node().has_error());
//! ```
//!
//! [Parser]: https://docs.rs/tree-sitter/*/tree_sitter/struct.Parser.html
//! [tree-sitter]: https://tree-sitter.github.io/

use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_freemarker() -> *const ();
}

/// The tree-sitter [`LanguageFn`][LanguageFn] for this grammar.
///
/// [LanguageFn]: https://docs.rs/tree-sitter-language/*/tree_sitter_language/struct.LanguageFn.html
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_freemarker) };

/// The content of the [`node-types.json`][] file for this grammar.
///
/// [`node-types.json`]: https://tree-sitter.github.io/tree-sitter/using-parsers#static-node-types
pub const NODE_TYPES: &str = include_str!("../../src/node-types.json");

// NOTE: uncomment these to include any queries that this grammar contains:

// pub const INJECTIONS_QUERY: &str = include_str!("../../queries/injections.scm");
pub const LOCALS_QUERY: &str = include_str!("../../queries/locals.scm");
pub const TAGS_QUERY: &str = include_str!("../../queries/tags.scm");

// const literals
pub const SEMANTICS: &str = "freemarker semantics";
pub const SYNTAX: &str = "freemarker syntax";

// extra public mods
pub mod grammar; // expose grammar rules via codegen
pub mod href;

#[cfg(test)]
mod tests {
    use crate::grammar::Rule;
    use std::str::FromStr;

    #[test]
    fn test_can_load_grammar() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&super::LANGUAGE.into())
            .expect("Error loading Freemarker parser");
    }

    #[test]
    fn test_freemarker_rule() {
        let rule_variant = Rule::from_str("nokia");
        assert!(rule_variant.is_err());
        let assign_clause = Rule::from_str("assign_clause").unwrap();
        assert_eq!(assign_clause, Rule::AssignClause);
        assert_eq!(assign_clause.to_string(), "assign_clause");
    }
}
