// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

use std::str::FromStr;

use tower_lsp_server::{
    jsonrpc::Result as JsonRpcResult,
    ls_types::{
        DefinitionOptions, GotoDefinitionParams, GotoDefinitionResponse, Location, OneOf, Range,
        Uri,
    },
};
use tree_sitter::Point;
use tree_sitter_freemarker::grammar::Rule;

use crate::{doc::TextDocument, protocol::Goto, symbol::MacroNamespace};

pub fn definition_capability() -> OneOf<bool, DefinitionOptions> {
    OneOf::Left(true)
}

impl Goto for TextDocument {
    async fn on_goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> JsonRpcResult<Option<GotoDefinitionResponse>> {
        let ast = self.tree.as_ref().expect("ast should not be None");
        let root = ast.root_node();
        let source = &self.rope.to_string();
        let point = Point {
            row: params.text_document_position_params.position.line as usize,
            column: params.text_document_position_params.position.character as usize,
        };
        let node = root.named_descendant_for_point_range(point, point);
        if node.is_none() {
            return Ok(None);
        }
        let node = node.unwrap();
        if node.is_error() || node.is_missing() {
            return Ok(None);
        }
        let rule = Rule::from_str(node.kind());
        if rule.is_err() {
            return Ok(None);
        }
        let rule = rule.unwrap();
        if rule == Rule::ImportPath {
            // the tree-sitter parser had ensured the import_path is '"' quoted
            // so it is safe to slice via [1..len()-1]
            let filepath = &source[node.start_byte() + 1..node.end_byte() - 1];
            if let Some(file_uri) = self.analyze_result.valid_imports.get(filepath) {
                tracing::debug!("goto import: {}", filepath);
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: file_uri.clone(),
                    range: Range::default(),
                })));
            }
        } else if rule == Rule::MacroNamespace {
            let namespace = &source[node.start_byte()..node.end_byte()];
            if let Some(macro_namespace) = self.analyze_result.macro_map.get(namespace) {
                match macro_namespace {
                    MacroNamespace::Local(local_macro) => {
                        return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                            uri: self.uri.clone(),
                            range: local_macro.alias_range,
                        })));
                    }
                    MacroNamespace::Import(import_macro) => {
                        if import_macro.path_valid {
                            let abs_import_path = self.import_path_to_absolute(&import_macro.path);
                            tracing::debug!("goto import: {:?}", abs_import_path);
                            if let Some(import_uri) = Uri::from_file_path(abs_import_path) {
                                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                                    uri: import_uri,
                                    range: Range::default(),
                                })));
                            }
                        } else {
                            return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                                // goto macro alias if the macro namespace is from a valid import path
                                uri: self.uri.clone(),
                                range: import_macro.alias_range,
                            })));
                        }
                    }
                }
            }
        }
        Ok(None)
    }
}
