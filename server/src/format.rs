// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

use tower_lsp_server::{
    jsonrpc::Result as JsonRpcResult,
    ls_types::{
        DocumentFormattingOptions, DocumentFormattingParams, OneOf, Position, Range, TextEdit,
    },
};
use tree_sitter::{Node, Point};

use crate::doc::TextDocument;
use crate::protocol::Formatter;

struct FormatState {
    preset: Option<usize>,
    indent: usize,
    has_directive: bool,
}

fn get_first_node_of_line<'a>(root: &'a Node<'a>, col: usize, index: usize) -> Node<'a> {
    let start = Point {
        row: index,
        column: col,
    };
    let end = Point {
        row: index,
        column: col + 2,
    };
    root.descendant_for_point_range(start, end).unwrap()
}

fn reset_state(mut state: FormatState) -> FormatState {
    // MUST NOT reset the preset
    state.indent = 0;
    state.has_directive = false;
    state
}

fn update_state(root: &Node, index: usize, line: &str, mut state: FormatState) -> FormatState {
    let trimed_line = line.trim_start();
    if trimed_line.starts_with("</#") || trimed_line.starts_with("<#") {
        let col = line.len() - trimed_line.len();
        let node = get_first_node_of_line(root, col, index);
        if node.kind() == "comment" {
            // under comment section
            state.has_directive = false;
        } else {
            state.has_directive = true;
            // compute indent
            let mut node_cursor = node;
            while let Some(parent) = node_cursor.parent() {
                let kind = node_cursor.kind();
                if kind.ends_with("_clause") {
                    // node kind with "_clause" requires indent increasing
                    state.indent += 1;
                }
                node_cursor = parent;
            }
            // save the preset indentation for the top level directives
            if state.indent == 0 {
                if let Some(parent) = node.parent() {
                    let _parentkind = parent.kind();
                    let end_row = parent.end_position().row;
                    let start_row = parent.start_position().row;
                    if end_row > start_row {
                        if state.preset.is_none() {
                            state.preset = Some(col);
                        }
                    } else if col > 0 {
                        state.preset = Some(col);
                    } else {
                        state.preset = None;
                    }
                }
            }
        }
    }
    state
}

fn format_source(root: &Node, source: &str) -> Vec<TextEdit> {
    let mut state = FormatState {
        preset: None,
        indent: 0,
        has_directive: false,
    };
    let mut formatted = String::from("");
    let lines = source.lines();
    let mut last_length = 0;
    for (index, line) in lines.into_iter().enumerate() {
        last_length = line.len();
        state = update_state(root, index, line, state);
        let preset = state.preset.unwrap_or_default();
        if state.has_directive {
            // todo: make indent step become a configuration
            // currently use 4 whitespaces as the indent step by default
            formatted += &(" ".repeat(preset + state.indent * 4) + line.trim() + "\n");
        } else {
            formatted += &(line.to_owned() + "\n");
        }
        state = reset_state(state);
    }
    let range = Range::new(
        Position::new(0, 0),
        Position::new(source.lines().count() as u32, last_length as u32),
    );
    vec![TextEdit::new(range, formatted)]
}

pub fn formatting_capability() -> OneOf<bool, DocumentFormattingOptions> {
    OneOf::Left(true)
}

impl Formatter for TextDocument {
    async fn on_formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> JsonRpcResult<Option<Vec<TextEdit>>> {
        let _ = params;
        let ast = self.tree.as_ref().expect("ast should not be None");
        let root = ast.root_node();
        let source = &self.rope.to_string();
        let result = format_source(&root, source);
        Ok(Some(result))
    }
}
