// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

use once_cell::sync::Lazy;
use rust_embed::Embed;
use serde::Deserialize;
use std::{collections::HashMap, str::FromStr};
use tower_lsp_server::{
    jsonrpc::Result as JsonRpcResult,
    ls_types::{
        Hover, HoverContents, HoverParams, HoverProviderCapability, LanguageString, MarkedString,
        MarkupContent, MarkupKind,
    },
};
use tree_sitter::Point;
use tree_sitter_freemarker::grammar::Rule;

use crate::doc::TextDocument;
use crate::symbol::MacroNamespace;
use crate::{protocol::Hovering, utils};

#[derive(Embed)]
#[folder = "assets/hover"]
struct HoverAssetPath;

#[derive(Debug, Default, Deserialize)]
struct HoverAssetItem {
    // static markdown text
    identifier: String,
    category: String,
    markdown: Option<String>,
    // TODO: dynamic text "rendering"
}

impl HoverAssetItem {
    #[tracing::instrument(skip_all)]
    fn from_bytes(bytes: &[u8]) -> Option<HoverAssetItem> {
        match std::str::from_utf8(bytes) {
            Ok(s) => match toml::from_str::<HoverAssetItem>(s) {
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
    fn from_embed(file: &str) -> Option<HoverAssetItem> {
        if let Some(completion_file) = HoverAssetPath::get(file) {
            return HoverAssetItem::from_bytes(completion_file.data.as_ref());
        }
        tracing::error!("rust-embed file not found: {}:", file);
        None
    }
}

#[derive(Debug, Clone)]
struct HoverAsset {
    built_in: HashMap<String, Hover>,
    // TODO: other hovers
}

impl HoverAsset {
    fn new() -> Self {
        let mut built_in: HashMap<String, Hover> = HashMap::new();
        HoverAssetPath::iter().for_each(|file| {
            if let Some(item) = HoverAssetItem::from_embed(&file) {
                if item.category.as_str() == "built-in" {
                    built_in.insert(
                        item.identifier,
                        Hover {
                            contents: HoverContents::Markup(MarkupContent {
                                kind: MarkupKind::Markdown,
                                value: item.markdown.unwrap_or_default(),
                            }),
                            range: None,
                        },
                    );
                }
            }
        });
        HoverAsset { built_in }
    }
}

static STATIC_ASSETS: Lazy<HoverAsset> = Lazy::new(HoverAsset::new);

pub fn hover_capability() -> HoverProviderCapability {
    HoverProviderCapability::Simple(true)
}

impl Hovering for TextDocument {
    async fn on_hover(&self, params: HoverParams) -> JsonRpcResult<Option<Hover>> {
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
        let identifier = &source[node.start_byte()..node.end_byte()];
        match rule {
            Rule::BuiltinName => {
                if let Some(hover) = STATIC_ASSETS.built_in.get(identifier) {
                    return Ok(Some(Hover {
                        contents: hover.contents.clone(),
                        range: Some(utils::parser_node_to_document_range(&node)),
                    }));
                }
                return Ok(None);
            }
            Rule::MacroNamespace => {
                if let Some(macro_namespace) = self.analyze_result.macro_map.get(identifier) {
                    match macro_namespace {
                        MacroNamespace::Local(_) => return Ok(None), // TODO: hover on local macro
                        MacroNamespace::Import(imported_macro) => {
                            return Ok(Some(Hover {
                                contents: HoverContents::Scalar(MarkedString::LanguageString(
                                    LanguageString {
                                        // javascript has syntax highlighting for "import ... as ..."
                                        language: "javascript".to_owned(),
                                        value: format!(
                                            r#"import "{}" as {}"#,
                                            imported_macro.path, identifier
                                        ),
                                    },
                                )),
                                range: Some(utils::parser_node_to_document_range(&node)),
                            }));
                        }
                    }
                }
            }
            _ => (),
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use crate::hover::{HoverAsset, HoverAssetItem};

    #[test]
    fn test_asset_builtin_from_str() {
        if let Some(item) = HoverAssetItem::from_bytes(
            r#"identifier = "foo"
category = "bar"
markdown = """baz"""
"#
            .as_bytes(),
        ) {
            assert_eq!(item.identifier, "foo".to_string());
            assert_eq!(item.category, "bar".to_string());
            assert!(item.markdown.is_some());
            assert_eq!(item.markdown.unwrap(), "baz".to_string());
        }
    }

    #[test]
    fn test_asset_builtin_from_file() {
        if let Some(item) = HoverAssetItem::from_embed("c.toml") {
            assert_eq!(item.identifier, "c".to_string());
            assert_eq!(item.category, "built-in".to_string());
            assert!(item.markdown.is_some());
        }
    }

    #[test]
    fn test_asset_builtin() {
        let asset = HoverAsset::new();
        assert!(!asset.built_in.is_empty());
    }
}
