// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

//! https://gist.github.com/rojas-diego/04d9c4e3fff5f8374f29b9b738d541ef

use std::path::{Path, PathBuf};

use ropey::{Rope, RopeSlice};
use thiserror::Error;
use tower_lsp_server::ls_types::{Position, TextDocumentContentChangeEvent, Uri};
use tree_sitter::{InputEdit, Parser, Point, Tree};

use crate::{
    analysis::{self, Analysis, AstAnalyzer},
    diagnosis::DiagnosticAnalyzer,
    folding::FoldingRangeAnalyzer,
    symbol::SymbolAnalyzer,
    tokenizer::SemanticTokenAnalyzer,
};

pub struct TextDocument {
    pub rope: Rope,
    pub tree: Option<Tree>,
    pub version: i32,
    parser: Parser,
    pub(crate) uri: Uri,
    pub(crate) analyze_result: Analysis,
}

#[derive(Error, Debug)]
pub enum DocumentError {
    #[error("position {0}:{1} is out of bounds")]
    PositionOutOfBounds(u32, u32),
}

#[derive(Clone, Debug, Copy)]
/// We redeclare this enum here because the `lsp_types` crate exports a Cow
/// type that is unconvenient to deal with.
pub enum PositionEncodingKind {
    UTF8,
    #[allow(dead_code)]
    UTF16,
    #[allow(dead_code)]
    UTF32,
}

impl TextDocument {
    /// Creates a new document from the given text and language id. It creates
    /// a rope, parser and syntax tree from the text.
    pub fn new(uri: &Uri, text: &str, version: i32) -> Self {
        let rope = Rope::from_str(text);
        let mut parser = Parser::new();
        let language = tree_sitter_freemarker::LANGUAGE;
        parser
            .set_language(&language.into())
            .expect("set parser language should always succeed");
        let tree = parser.parse(text, None);
        let mut doc = TextDocument {
            rope,
            tree,
            version,
            parser,
            uri: uri.clone(),
            analyze_result: Default::default(),
        };
        // internal do analyze
        doc.analyze_result = doc.do_analyze();
        doc
    }

    /// Apply a change to the document.
    pub fn apply_content_change(
        &mut self,
        new_version: i32,
        change: &TextDocumentContentChangeEvent,
        position_encoding: PositionEncodingKind,
    ) -> Result<(), DocumentError> {
        match change.range {
            Some(range) => {
                assert!(
                    range.start.line < range.end.line
                        || (range.start.line == range.end.line
                            && range.start.character <= range.end.character)
                );

                let same_line = range.start.line == range.end.line;
                let same_character = range.start.character == range.end.character;

                let change_start_line_cu_idx = range.start.character as usize;
                let change_end_line_cu_idx = range.end.character as usize;

                // 1. Get the line at which the change starts.
                let change_start_line_idx = range.start.line as usize;
                let change_start_line = match self.rope.get_line(change_start_line_idx) {
                    Some(line) => line,
                    None => {
                        return Err(DocumentError::PositionOutOfBounds(
                            range.start.line,
                            range.start.character,
                        ));
                    }
                };

                // 2. Get the line at which the change ends. (Small optimization
                // where we first check whether start and end line are the
                // same O(log N) lookup. We repeat this all throughout this
                // function).
                let change_end_line_idx = range.end.line as usize;
                let change_end_line = match same_line {
                    true => change_start_line,
                    false => match self.rope.get_line(change_end_line_idx) {
                        Some(line) => line,
                        None => {
                            return Err(DocumentError::PositionOutOfBounds(
                                range.end.line,
                                range.end.character,
                            ));
                        }
                    },
                };

                fn compute_char_idx(
                    position_encoding: PositionEncodingKind,
                    position: &Position,
                    slice: &RopeSlice,
                ) -> Result<usize, DocumentError> {
                    match position_encoding {
                        PositionEncodingKind::UTF8 => {
                            slice.try_byte_to_char(position.character as usize)
                        }
                        PositionEncodingKind::UTF16 => {
                            slice.try_utf16_cu_to_char(position.character as usize)
                        }
                        PositionEncodingKind::UTF32 => Ok(position.character as usize),
                    }
                    .map_err(|_| {
                        DocumentError::PositionOutOfBounds(position.line, position.character)
                    })
                }

                // 3. Compute the character offset into the start/end line where
                // the change starts/ends.
                let change_start_line_char_idx =
                    compute_char_idx(position_encoding, &range.start, &change_start_line)?;
                let change_end_line_char_idx = match same_line && same_character {
                    true => change_start_line_char_idx,
                    false => compute_char_idx(position_encoding, &range.end, &change_end_line)?,
                };

                // 4. Compute the character and byte offset into the document
                // where the change starts/ends.
                let change_start_doc_char_idx =
                    self.rope.line_to_char(change_start_line_idx) + change_start_line_char_idx;
                let change_end_doc_char_idx = match same_line && same_character {
                    true => change_start_doc_char_idx,
                    false => self.rope.line_to_char(change_end_line_idx) + change_end_line_char_idx,
                };
                let change_start_doc_byte_idx = self.rope.char_to_byte(change_start_doc_char_idx);
                let change_end_doc_byte_idx = match same_line && same_character {
                    true => change_start_doc_byte_idx,
                    false => self.rope.char_to_byte(change_end_doc_char_idx),
                };

                // 5. Compute the byte offset into the start/end line where the
                // change starts/ends. Required for tree-sitter.
                let change_start_line_byte_idx = match position_encoding {
                    PositionEncodingKind::UTF8 => change_start_line_cu_idx,
                    PositionEncodingKind::UTF16 => {
                        change_start_line.char_to_utf16_cu(change_start_line_char_idx)
                    }
                    PositionEncodingKind::UTF32 => change_start_line_char_idx,
                };
                let change_end_line_byte_idx = match same_line && same_character {
                    true => change_start_line_byte_idx,
                    false => match position_encoding {
                        PositionEncodingKind::UTF8 => change_end_line_cu_idx,
                        PositionEncodingKind::UTF16 => {
                            change_end_line.char_to_utf16_cu(change_end_line_char_idx)
                        }
                        PositionEncodingKind::UTF32 => change_end_line_char_idx,
                    },
                };

                self.rope
                    .remove(change_start_doc_char_idx..change_end_doc_char_idx);
                self.rope.insert(change_start_doc_char_idx, &change.text);

                if let Some(tree) = &mut self.tree {
                    // 6. Compute the byte index into the new end line where the
                    // change ends. Required for tree-sitter.
                    let change_new_end_line_idx = self
                        .rope
                        .byte_to_line(change_start_doc_byte_idx + change.text.len());
                    let change_new_end_line_byte_idx =
                        change_start_doc_byte_idx + change.text.len();

                    // 7. Construct the tree-sitter edit. We stay mindful that
                    // tree-sitter Point::column is a byte offset.
                    let edit = InputEdit {
                        start_byte: change_start_doc_byte_idx,
                        old_end_byte: change_end_doc_byte_idx,
                        new_end_byte: change_start_doc_byte_idx + change.text.len(),
                        start_position: Point {
                            row: change_start_line_idx,
                            column: change_start_line_byte_idx,
                        },
                        old_end_position: Point {
                            row: change_end_line_idx,
                            column: change_end_line_byte_idx,
                        },
                        new_end_position: Point {
                            row: change_new_end_line_idx,
                            column: change_new_end_line_byte_idx,
                        },
                    };

                    tree.edit(&edit);
                    self.tree = self.parser.parse(self.rope.to_string(), Some(tree));
                }
            }
            None => {
                self.rope = Rope::from_str(&change.text);
                self.tree = self.parser.parse(&change.text, None);
            }
        }
        // update version
        self.version = new_version;
        self.analyze_result = self.do_analyze();
        Ok(())
    }

    pub fn do_analyze(&mut self) -> Analysis {
        let ast = self.tree.as_ref().expect("not gonna happen!");
        let root = ast.root_node();
        // Create all the 'AstAnalyzer's
        let mut tk = SemanticTokenAnalyzer::new();
        let mut dg = DiagnosticAnalyzer::new();
        let mut fr = FoldingRangeAnalyzer::new();
        let mut sa = SymbolAnalyzer::new(&self.uri);
        // Generic 'AstAnalyzer' Vec
        let mut phase1: Vec<&mut dyn AstAnalyzer> = vec![&mut tk, &mut fr, &mut sa];
        let mut phase2: Vec<&mut dyn AstAnalyzer> = vec![&mut dg];
        let source = &self.rope.to_string();
        let mut analysis = Analysis {
            ..Default::default()
        };
        analysis::do_analyze(&root, source, &mut phase1, &mut analysis);
        analysis::do_analyze(&root, source, &mut phase2, &mut analysis);
        // AST analyzing completed
        if let Some(symbol_diagnostic) = sa.diagnostic {
            // TODO: merge dg.report.related_documents
            analysis
                .diagnostic
                .full_document_diagnostic_report
                .items
                .extend(symbol_diagnostic.full_document_diagnostic_report.items);
        }
        analysis
    }

    pub fn import_path_to_absolute(&self, import_path: &str) -> PathBuf {
        let file_path = Path::new(import_path);
        match file_path.is_absolute() {
            true => PathBuf::from(import_path),
            false => {
                // relative directory is relative to current file?
                let self_binding = self.uri.to_file_path().unwrap();
                let base_dir = self_binding.parent().unwrap();
                let rest = PathBuf::from(import_path);
                base_dir.join(rest)
            }
        }
    }
}
