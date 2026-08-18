// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

use tree_sitter::{InputEdit, Node, Parser, Point, Tree};

pub struct TextParser {
    parser: Parser,
    ast: Option<Tree>,
}

impl std::fmt::Debug for TextParser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextParser")
            .field("has_ast", &self.ast.is_some())
            .finish()
    }
}

impl TextParser {
    /// Creates a new parser and parses the given text into an initial syntax tree.
    pub fn new(text: &str) -> Self {
        let mut parser = Self::parser_for_freemarker();
        let ast = parser.parse(text, None);
        TextParser { parser, ast }
    }

    fn parser_for_freemarker() -> Parser {
        let mut parser = Parser::new();
        let language = tree_sitter_freemarker::LANGUAGE;
        parser
            .set_language(&language.into())
            .expect("set parser language should always succeed");
        parser
    }

    pub fn get_ast(&self) -> Option<Tree> {
        self.ast.clone()
    }

    pub fn get_node_at_point(&self, point: Point) -> Option<Node<'_>> {
        let tree = self.ast.as_ref()?;
        tree.root_node()
            .named_descendant_for_point_range(point, point)
    }

    pub fn apply_edit(&mut self, text: &str, input_edit: Option<InputEdit>) {
        // TODO: what if the document's encoding is not UTF8?
        // Apply the incremental edit to the existing tree, then reuse it for an
        // incremental re-parse. A `None` edit means the whole document changed,
        // in which case we fall back to a full parse.
        let mut incremental = false;
        if let Some(edit) = input_edit {
            let tree = self
                .ast
                .as_mut()
                .expect("cannot apply an incremental edit without an existing tree");
            tree.edit(&edit);
            incremental = true;
        }
        let old_tree = if incremental { self.ast.as_ref() } else { None };
        self.ast = self.parser.parse(text, old_tree);
    }
}
