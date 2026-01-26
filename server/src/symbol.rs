// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    str::FromStr,
};

use tower_lsp_server::ls_types::{
    CodeDescription, Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity,
    FullDocumentDiagnosticReport, Location, NumberOrString, Range,
    RelatedFullDocumentDiagnosticReport, Uri,
};
use tree_sitter::Node;
use tree_sitter_freemarker::href::DIRECTIVE_IMPORT;
use tree_sitter_freemarker::{SEMANTICS, grammar::Rule};

use crate::{
    analysis::{Analysis, AstAnalyzer},
    utils,
};

#[derive(Debug, Clone)]
pub struct ImportMacro {
    pub alias_range: Range,
    pub path: String,
    pub path_range: Range,
    pub path_valid: bool,
}

#[derive(Debug, Clone)]
pub struct LocalMacro {
    pub alias_range: Range,
    pub row: usize,
}

#[derive(Debug, Clone)]
pub enum MacroNamespace {
    Local(LocalMacro),
    Import(ImportMacro),
}

pub struct SymbolAnalyzer {
    uri: Uri,
    pub import_list: Vec<ImportMacro>,
    pub path_map: HashMap<String, usize>,
    pub diagnostic: Option<RelatedFullDocumentDiagnosticReport>,
}

impl SymbolAnalyzer {
    pub fn new(uri: &Uri) -> Self {
        SymbolAnalyzer {
            uri: uri.clone(),
            import_list: vec![],
            path_map: HashMap::new(),
            diagnostic: None,
        }
    }

    fn add_diagnostic_item(&mut self, item: Diagnostic) {
        match &mut self.diagnostic {
            Some(report) => {
                report.full_document_diagnostic_report.items.push(item);
            }
            None => {
                self.diagnostic = Some(RelatedFullDocumentDiagnosticReport {
                    related_documents: None,
                    full_document_diagnostic_report: FullDocumentDiagnosticReport {
                        result_id: None, // TODO: handle version
                        items: vec![item],
                    },
                });
            }
        }
    }

    #[tracing::instrument(skip_all)]
    fn analyze_import(
        &mut self,
        path_node: &Node,
        alias_node: &Node,
        source: &str,
        analysis: &mut Analysis,
    ) {
        // the tree-sitter parser had ensured the import_path is '"' quoted, so it is safe to slice like this [1..len()-1]
        let import_path = &source[path_node.start_byte() + 1..path_node.end_byte() - 1];
        let import_alias = &source[alias_node.start_byte()..alias_node.end_byte()];
        let path_range = utils::node_range(path_node);
        let alias_range = utils::node_range(alias_node);
        // Step1: file valid check
        let file_path = Path::new(import_path);
        let abs_import_path = match file_path.is_absolute() {
            true => PathBuf::from(import_path),
            false => {
                // relative directory is relative to current file?
                let self_binding = self.uri.to_file_path().unwrap();
                let base_dir = self_binding.parent().unwrap();
                let rest = PathBuf::from(import_path);
                base_dir.join(rest)
            }
        };
        let file_is_valid = match abs_import_path.is_file() {
            true => true,
            false => {
                let (error_code, error_message) = match abs_import_path.exists() {
                    true => ("import_path_not_file", "import path is not a file"),
                    false => ("import_path_not_exist", "import path does not exist"),
                };
                self.add_diagnostic_item(Diagnostic {
                    range: path_range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String(error_code.to_owned())),
                    code_description: Some(CodeDescription {
                        href: DIRECTIVE_IMPORT.parse().unwrap(),
                    }),
                    source: Some(SEMANTICS.to_owned()),
                    message: error_message.to_string(),
                    ..Default::default()
                });
                false
            }
        };
        // Step0, the import-stmt MUST to be recorded
        let import_macro = ImportMacro {
            alias_range,
            path: import_path.to_owned(),
            path_range,
            path_valid: file_is_valid,
        };
        let import_index = self.import_list.len();
        // push to [macro] list
        self.import_list.push(import_macro.clone());
        // Step2: do not self-import
        if file_is_valid {
            let import_binding = abs_import_path.canonicalize().unwrap();
            let canonical_import_path = import_binding.to_str().unwrap();
            tracing::debug!("canonical_import_path is {:?}", canonical_import_path);
            let self_binding = self.uri.to_file_path().unwrap();
            let canonical_self_path = self_binding.to_str().unwrap();
            if canonical_import_path == canonical_self_path {
                self.add_diagnostic_item(Diagnostic {
                    range: path_range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("self_import".to_owned())),
                    code_description: Some(CodeDescription {
                        href: DIRECTIVE_IMPORT.parse().unwrap(),
                    }),
                    source: Some(SEMANTICS.to_owned()),
                    // TODO: need to check circular import?
                    message: "do not import the template itself".to_string(),
                    ..Default::default()
                });
            } else {
                // save raw import path
                analysis.valid_imports.insert(
                    import_path.to_owned(),
                    Uri::from_file_path(canonical_import_path).unwrap(),
                );
            }

            // Step3: path duplication check
            if self.path_map.contains_key(canonical_import_path) {
                let first_import_index = self.path_map.get(canonical_import_path).unwrap();
                let first_import = &self.import_list[*first_import_index];
                self.add_diagnostic_item(Diagnostic {
                    range: path_range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: Some(NumberOrString::String("import_path_duplicated".to_owned())),
                    code_description: Some(CodeDescription {
                        href: DIRECTIVE_IMPORT.parse().unwrap(),
                    }),
                    source: Some(SEMANTICS.to_owned()),
                    message: "import path is duplicated".to_string(),
                    related_information: Some(vec![DiagnosticRelatedInformation {
                        location: Location {
                            uri: self.uri.clone(),
                            range: first_import.path_range,
                        },
                        message: "first imported here".to_owned(),
                    }]),
                    ..Default::default()
                });
            } else {
                // new path, add to [path, macro] map
                self.path_map
                    .insert(canonical_import_path.to_owned(), import_index);
            }
        }

        // Step4: alias duplication check
        if analysis.macro_map.contains_key(import_alias) {
            let first_define = analysis.macro_map.get(import_alias).unwrap();
            self.add_diagnostic_item(Diagnostic {
                range: alias_range,
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String(
                    "import_namespace_duplicated".to_owned(),
                )),
                code_description: Some(CodeDescription {
                    href: DIRECTIVE_IMPORT.parse().unwrap(),
                }),
                source: Some(SEMANTICS.to_owned()),
                message: "import namespace is duplicated".to_string(),
                related_information: Some(vec![DiagnosticRelatedInformation {
                    location: Location {
                        uri: self.uri.clone(),
                        range: match first_define {
                            MacroNamespace::Local(local_name) => local_name.alias_range,
                            MacroNamespace::Import(import_name) => import_name.alias_range,
                        },
                    },
                    message: "first defined here".to_owned(),
                }]),
                ..Default::default()
            });
        } else {
            // add to [alias, macro] table
            analysis.macro_map.insert(
                import_alias.to_owned(),
                MacroNamespace::Import(import_macro),
            );
        }
    }
}

impl AstAnalyzer for SymbolAnalyzer {
    fn analyze_node(&mut self, node: &Node, source: &str, analysis: &mut Analysis) {
        let rule = Rule::from_str(node.kind());
        if rule.is_err() {
            return;
        }
        match rule.unwrap() {
            Rule::ImportStmt => {
                // hardcoded according to tree-sitter-freemarker/grammar.js
                let import_path_node = node
                    .child_by_field_name(Rule::ImportPath.to_string())
                    .unwrap();
                let import_alias_node = node
                    .child_by_field_name(Rule::ImportAlias.to_string())
                    .unwrap();
                self.analyze_import(&import_path_node, &import_alias_node, source, analysis);
            }
            Rule::MacroName => {
                let macro_name = &source[node.start_byte()..node.end_byte()];
                let node_range = utils::node_range(node);
                // TODO: fake import, improve it
                if analysis.macro_map.contains_key(macro_name) {
                    let first_define = analysis.macro_map.get(macro_name).unwrap();
                    self.add_diagnostic_item(Diagnostic {
                        range: node_range,
                        severity: Some(DiagnosticSeverity::ERROR),
                        code: Some(NumberOrString::String(
                            "import_namespace_duplicated".to_owned(),
                        )),
                        code_description: Some(CodeDescription {
                            href: DIRECTIVE_IMPORT.parse().unwrap(),
                        }),
                        source: Some(SEMANTICS.to_owned()),
                        message: "import namespace is duplicated".to_string(),
                        related_information: Some(vec![DiagnosticRelatedInformation {
                            location: Location {
                                uri: self.uri.clone(),
                                range: match first_define {
                                    MacroNamespace::Local(local_name) => local_name.alias_range,
                                    MacroNamespace::Import(import_name) => import_name.alias_range,
                                },
                            },
                            message: "first defined here".to_owned(),
                        }]),
                        ..Default::default()
                    });
                } else {
                    analysis.macro_map.insert(
                        macro_name.to_owned(),
                        MacroNamespace::Local(LocalMacro {
                            alias_range: node_range,
                            row: node.start_position().row,
                        }),
                    );
                }
            }
            _ => {}
        }
    }
}
