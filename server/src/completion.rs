// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

use tower_lsp_server::{
    jsonrpc::Result as JsonRpcResult,
    ls_types::{
        CompletionItem, CompletionItemKind, CompletionItemLabelDetails, CompletionOptions,
        CompletionOptionsCompletionItem, CompletionParams, CompletionResponse, Documentation,
        InsertTextFormat, InsertTextMode, MarkupContent, MarkupKind,
    },
};

use once_cell::sync::Lazy;
use rust_embed::Embed;
use serde::Deserialize;
use strum::IntoEnumIterator;
use tree_sitter_freemarker::grammar::Builtin;

use crate::doc::TextDocument;
use crate::{protocol::Completion, symbol::MacroNamespace};

#[derive(Embed)]
#[folder = "assets/completion"]
struct CompletionAssetPath;

#[derive(Debug, Default, Deserialize)]
struct LabelDetails {
    detail: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct CompletionAssetItem {
    category: String,
    deprecated: Option<bool>,
    label: String,
    insert_text: String,
    documentation: String,
    label_details: Option<LabelDetails>,
}

impl CompletionAssetItem {
    #[tracing::instrument(skip_all)]
    fn from_bytes(bytes: &[u8]) -> Option<CompletionAssetItem> {
        match std::str::from_utf8(bytes) {
            Ok(s) => match toml::from_str::<CompletionAssetItem>(s) {
                Ok(item) => Some(item),
                Err(e) => {
                    panic!("rust-embed deserialization error: {}:", e)
                }
            },
            Err(e) => {
                panic!("utf-8 parsing error: {}", e)
            }
        }
    }

    #[tracing::instrument(skip_all)]
    fn from_embed(file: &str) -> Option<CompletionAssetItem> {
        if let Some(completion_file) = CompletionAssetPath::get(file) {
            return CompletionAssetItem::from_bytes(completion_file.data.as_ref());
        }
        tracing::error!("rust-embed file not found: {}:", file);
        None
    }

    fn as_directive_completion(&self) -> CompletionItem {
        assert_eq!(self.category, "directive");
        CompletionItem {
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: self.documentation.clone(),
            })),
            deprecated: self.deprecated,
            label: self.label.clone(),
            label_details: self
                .label_details
                .as_ref()
                .map(|ld| CompletionItemLabelDetails {
                    detail: ld.detail.clone(),
                    description: ld.description.clone(),
                }),
            kind: Some(CompletionItemKind::METHOD),
            insert_text: Some(self.insert_text.clone()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            insert_text_mode: Some(InsertTextMode::ADJUST_INDENTATION),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
struct CompletionAsset {
    directive_completion: Vec<CompletionItem>,
    // TODO: other completions
}

impl CompletionAsset {
    fn new() -> Self {
        let mut directive_completion: Vec<CompletionItem> = vec![];
        CompletionAssetPath::iter().for_each(|file| {
            if let Some(item) = CompletionAssetItem::from_embed(&file) {
                if item.category.as_str() == "directive" {
                    directive_completion.push(item.as_directive_completion())
                }
            }
        });
        CompletionAsset {
            directive_completion,
        }
    }
}

static STATIC_ASSETS: Lazy<CompletionAsset> = Lazy::new(CompletionAsset::new);

fn completion_for_builtin() -> Vec<CompletionItem> {
    // todo: improve filter result by partial identifier
    Builtin::iter()
        .map(|i| CompletionItem {
            label: i.to_string(),
            kind: Some(CompletionItemKind::FIELD),
            ..Default::default()
        })
        .collect()
}

pub fn completion_capability() -> CompletionOptions {
    CompletionOptions {
        resolve_provider: Some(false),
        trigger_characters: Some(vec![
            "#".to_string(), // '<#' --> trigger directive
            "{".to_string(), // '${' --> trigger interpolation
            "?".to_string(), // '?' --> trigger built-ins
            "@".to_string(), // "<@" --> trigger macro call
        ]),
        all_commit_characters: None,
        work_done_progress_options: Default::default(),
        completion_item: Some(CompletionOptionsCompletionItem {
            label_details_support: Some(true),
        }),
    }
}

impl Completion for TextDocument {
    async fn on_completion(
        &self,
        params: CompletionParams,
    ) -> JsonRpcResult<Option<CompletionResponse>> {
        let position = params.text_document_position.position;
        let source = &self.rope.to_string();
        // in rust how can I get the (row, col) character from a String
        let mut lines = source.lines();
        let line = lines.nth(position.line as usize).unwrap();
        let prev = match position.character > 1 {
            true => line.chars().nth(position.character as usize - 2),
            false => None,
        };
        let mut result: Option<CompletionResponse> = None;
        if params.context.is_some_and(|c| {
            c.trigger_character.is_some_and(|trigger| {
                match trigger.as_str() {
                    "#" => {
                        if prev.is_some_and(|c| c == '<') {
                            // triggered by '<#', expect a directive keyword
                            result = Some(CompletionResponse::Array(
                                STATIC_ASSETS.directive_completion.clone(),
                            ));
                            return true;
                        }
                        false
                    }
                    "@" => {
                        if prev.is_some_and(|c| c == '<') {
                            // triggered by '<@', expect a macro call
                            let imported_macros: Vec<CompletionItem> = self
                                .analyze_result
                                .macro_map
                                .iter()
                                .map(|(macro_name, macro_item)| CompletionItem {
                                    label: macro_name.to_owned(),
                                    kind: Some(CompletionItemKind::MODULE),
                                    documentation: Some(Documentation::MarkupContent(
                                        MarkupContent {
                                            kind: MarkupKind::Markdown,
                                            value: match macro_item {
                                                MacroNamespace::Local(local_macro) => {
                                                    let source_line =
                                                        lines.nth(local_macro.row).unwrap();
                                                    source_line.to_string()
                                                }
                                                MacroNamespace::Import(import_macro) => {
                                                    format!(
                                                        "```python\nimport \"{}\" as {}\n```",
                                                        import_macro.path, macro_name
                                                    )
                                                }
                                            },
                                        },
                                    )),
                                    insert_text: Some(macro_name.to_owned()),
                                    insert_text_format: Some(InsertTextFormat::SNIPPET),
                                    insert_text_mode: Some(InsertTextMode::AS_IS),
                                    ..Default::default()
                                })
                                .collect();
                            result = Some(CompletionResponse::Array(imported_macros));
                            return true;
                        }
                        false
                    }
                    "?" => {
                        // triggered by '?', expect a built-in
                        result = Some(CompletionResponse::Array(completion_for_builtin()));
                        true
                    }
                    _ => false,
                }
            })
        }) {
            // trigger character is typed, but which might not need to
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use crate::completion::{CompletionAsset, CompletionAssetItem};

    #[test]
    fn test_asset_assign_directive() {
        let item = CompletionAssetItem::from_embed("assign.toml");
        assert!(item.is_some());
        let item = item.unwrap();
        assert_eq!(item.category.as_str(), "directive");
        assert_eq!(item.label.as_str(), "assign");
        assert!(item.label_details.is_none());
    }

    #[test]
    fn test_asset_assign_capture_directive() {
        let item = CompletionAssetItem::from_embed("assign(capture).toml");
        assert!(item.is_some());
        let item = item.unwrap();
        assert_eq!(item.category.as_str(), "directive");
        assert_eq!(item.label.as_str(), "assign");
        match item.label_details {
            Some(label_details) => {
                assert_eq!(label_details.detail.unwrap_or_default(), "(capture)");
                assert!(label_details.description.is_none());
            }
            None => assert!(false),
        }
    }

    #[test]
    fn test_asset_directives() {
        let asset = CompletionAsset::new();
        assert!(!asset.directive_completion.is_empty());
    }
}
