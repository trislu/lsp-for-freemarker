// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

use std::str::FromStr;
use tower_lsp_server::{
    jsonrpc,
    ls_types::{
        CodeDescription, Diagnostic, DiagnosticOptions, DiagnosticServerCapabilities,
        DiagnosticSeverity, DocumentDiagnosticParams, DocumentDiagnosticReport,
        DocumentDiagnosticReportResult, NumberOrString, Position, Range,
    },
};
use tree_sitter::Node;
use tree_sitter_freemarker::{
    SEMANTICS, SYNTAX,
    href::{DIRECTIVE_ASSIGN, DIRECTIVE_IMPORT, DIRECTIVE_LIST_BREAK, TOPLEVEL_VARIABLE},
};
use tree_sitter_freemarker::{grammar::Rule, href::COMPARISION_EXPRESSION};

use crate::{
    analysis::{Analysis, AstAnalyzer},
    doc::TextDocument,
    protocol::Diagnose,
    utils,
};

pub fn diagnostic_capability() -> DiagnosticServerCapabilities {
    DiagnosticServerCapabilities::Options(DiagnosticOptions {
        identifier: None,
        inter_file_dependencies: true,
        workspace_diagnostics: false,
        work_done_progress_options: Default::default(),
    })
}

pub struct Scenario {
    severity: DiagnosticSeverity,
    code: &'static str,
    source: &'static str,
    message: &'static str,
    href: &'static str,
}

impl Scenario {
    const UNDEFINED_MACRO: Scenario = Scenario {
        severity: DiagnosticSeverity::ERROR,
        code: "undefined_macro",
        source: SEMANTICS,
        message: "macro definition not found",
        href: DIRECTIVE_IMPORT,
    };

    const BACKSLASHED_IDENTIFIER: Scenario = Scenario {
        severity: DiagnosticSeverity::INFORMATION,
        code: "identifier_has_backslash",
        source: SYNTAX,
        message: "Reserved characters in the identifier must be escaped with a preceding backslash (\\), which decrease the readability, try best to avoid it.",
        href: TOPLEVEL_VARIABLE,
    };

    const AMBIGUOUS_STRING_LITERAL: Scenario = Scenario {
        severity: DiagnosticSeverity::WARNING,
        code: "ambiguous_string_literal",
        source: SYNTAX,
        message: "Even though it is a valid syntax for <#assign> and <#local>, using string literal as L-value is still toxic.",
        href: DIRECTIVE_ASSIGN,
    };

    const DEPRECATED_EQUAL_OPERATOR: Scenario = Scenario {
        severity: DiagnosticSeverity::WARNING,
        code: "deprecated_equal_operator",
        source: SYNTAX,
        message: "In the context of comparisons, use '==' for equality checks, '=' is a deprecated alternative.",
        href: COMPARISION_EXPRESSION,
    };

    const UNDOCUMENTED_CLOSE_TAG: Scenario = Scenario {
        severity: DiagnosticSeverity::WARNING,
        code: "undocumented_close_tag",
        source: SYNTAX,
        message: "Suggest using '>' as the close tag for non-capture <#assign>, '/>' is undocumented and wastes 1 more character.",
        href: DIRECTIVE_ASSIGN,
    };

    const DEPRECATED_LIST_BREAK: Scenario = Scenario {
        severity: DiagnosticSeverity::WARNING,
        code: "deprecated_list_break",
        source: SYNTAX,
        message: "break is deprecated for most use cases, as it doesn't work well with <#sep> and item?has_next. Instead, use sequence?take_while(predicate) to cut the sequence before you list it.",
        href: DIRECTIVE_LIST_BREAK,
    };

    const UNEXPECTED_BREAK_STMT: Scenario = Scenario {
        severity: DiagnosticSeverity::ERROR,
        code: "unexpected_break_stmt",
        source: SYNTAX,
        message: "<#break> can only be used within <#list> or <#switch>.",
        href: DIRECTIVE_LIST_BREAK,
    };
}

impl From<Scenario> for Diagnostic {
    fn from(s: Scenario) -> Self {
        Diagnostic {
            severity: Some(s.severity),
            code: Some(NumberOrString::String(s.code.to_owned())),
            code_description: Some(CodeDescription {
                href: s.href.parse().unwrap(),
            }),
            source: Some(s.source.to_owned()),
            message: s.message.to_owned(),
            ..Default::default()
        }
    }
}

pub struct DiagnosticAnalyzer {
    pub scope: Vec<Rule>,
}

impl DiagnosticAnalyzer {
    pub fn new() -> Self {
        DiagnosticAnalyzer { scope: vec![] }
    }

    fn diagnos_node(
        &mut self,
        node: &Node,
        code: &str,
        analysis: &mut Analysis,
    ) -> Option<Diagnostic> {
        let start_pos = node.start_position();
        let end_pos = node.end_position();
        let start = Position {
            line: start_pos.row as u32,
            character: start_pos.column as u32,
        };
        let end = Position {
            line: end_pos.row as u32,
            character: end_pos.column as u32,
        };
        let range: Range = Range { start, end };
        let node_kind = node.kind();
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();
        let snippet = &code[start_byte..end_byte];
        // TODO: maybe use tree-sitter query in the future
        if node.is_missing() {
            // TODO : maybe use query in the future
            return Some(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some(SYNTAX.to_owned()),
                message: format!("Missing {} here", node_kind),
                ..Default::default()
            });
        }

        if node.is_error() {
            return Some(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some(SYNTAX.to_owned()),
                message: format!("ERROR: Unexpected '{}'.\n", snippet),
                ..Default::default()
            });
        }

        if let Ok(rule) = Rule::from_str(node_kind) {
            match rule {
                Rule::Identifier => {
                    if snippet.contains("\\") {
                        return Some(Diagnostic {
                            range,
                            ..Scenario::BACKSLASHED_IDENTIFIER.into()
                        });
                    }
                }
                Rule::AmbiguousStringLiteral => {
                    return Some(Diagnostic {
                        range,
                        ..Scenario::AMBIGUOUS_STRING_LITERAL.into()
                    });
                }
                Rule::DeprecatedEqualOperator => {
                    return Some(Diagnostic {
                        range,
                        ..Scenario::DEPRECATED_EQUAL_OPERATOR.into()
                    });
                }
                Rule::UndocumentedCloseTag => {
                    return Some(Diagnostic {
                        range,
                        ..Scenario::UNDOCUMENTED_CLOSE_TAG.into()
                    });
                }
                Rule::ListBegin | Rule::SwitchBegin => {
                    self.scope.push(rule);
                }
                Rule::ListClose | Rule::SwitchClose => {
                    self.scope.pop();
                }
                Rule::BreakStmt => {
                    if let Some(s) = self.scope.last()
                        && *s == Rule::ListBegin
                    {
                        return Some(Diagnostic {
                            range,
                            ..Scenario::DEPRECATED_LIST_BREAK.into()
                        });
                    } else {
                        return Some(Diagnostic {
                            range,
                            ..Scenario::UNEXPECTED_BREAK_STMT.into()
                        });
                    }
                }
                Rule::MacroNamespace => {
                    if !analysis.macro_map.contains_key(snippet) {
                        return Some(Diagnostic {
                            range: utils::node_range(node),
                            ..Scenario::UNDEFINED_MACRO.into()
                        });
                    }
                }
                _ => {}
            }
        }
        None
    }
}

impl AstAnalyzer for DiagnosticAnalyzer {
    fn analyze_node(&mut self, node: &Node, source: &str, analysis: &mut Analysis) {
        if let Some(diagnostic) = self.diagnos_node(node, source, analysis) {
            analysis
                .diagnostic
                .full_document_diagnostic_report
                .items
                .push(diagnostic);
        }
    }
}

impl Diagnose for TextDocument {
    async fn on_diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> jsonrpc::Result<DocumentDiagnosticReportResult> {
        // TODO: Unchanged support
        let _ = params;
        Ok(DocumentDiagnosticReportResult::Report(
            DocumentDiagnosticReport::Full(self.analyze_result.diagnostic.clone()),
        ))
    }
}
