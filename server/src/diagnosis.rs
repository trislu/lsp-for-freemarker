// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

use std::str::FromStr;

use tower_lsp_server::{
    jsonrpc,
    ls_types::{
        CodeDescription, Diagnostic, DiagnosticOptions, DiagnosticServerCapabilities,
        DiagnosticSeverity, DocumentDiagnosticParams, DocumentDiagnosticReport,
        DocumentDiagnosticReportResult, NumberOrString,
    },
};
use tree_sitter::Node;
use tree_sitter_freemarker::{
    SEMANTICS, SYNTAX,
    grammar::Rule,
    href::{
        COMPARISION_EXPRESSION, DIRECTIVE_ASSIGN, DIRECTIVE_IMPORT, DIRECTIVE_LIST_BREAK,
        TOPLEVEL_VARIABLE,
    },
};

use crate::{
    analysis::{Analysis, AnalysisContext, DiagnosticAnalysis, Symbol},
    doc::TextDocument,
    reactor::Reactor,
    server::DiagnosticFeature,
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
    pub const UNDEFINED_MACRO: Scenario = Scenario {
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

impl DiagnosticAnalysis for Analysis {
    fn analyze_diagnostic_report(
        &mut self,
        node: &Node,
        doc: &TextDocument,
        ctx: &mut AnalysisContext,
    ) {
        let node_kind = node.kind();
        let range = utils::parser_node_to_document_range(node);
        // TODO: maybe use tree-sitter query in the future
        if node.is_missing() {
            // TODO : maybe use query in the future
            self.add_diagnostic(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some(SYNTAX.to_owned()),
                message: format!("Missing {} here", node_kind),
                ..Default::default()
            });
        }

        if node.is_error() {
            let node_text = doc.get_ranged_text(node.start_byte()..node.end_byte());
            self.add_diagnostic(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some(SYNTAX.to_owned()),
                message: format!("ERROR: Unexpected '{}'.\n", node_text),
                ..Default::default()
            });
        }

        if let Ok(rule) = Rule::from_str(node_kind) {
            match rule {
                Rule::Identifier => {
                    let node_text = doc.get_ranged_text(node.start_byte()..node.end_byte());
                    if node_text.contains("\\") {
                        self.add_diagnostic(Diagnostic {
                            range,
                            ..Scenario::BACKSLASHED_IDENTIFIER.into()
                        });
                    }
                }
                Rule::AmbiguousStringLiteral => {
                    self.add_diagnostic(Diagnostic {
                        range,
                        ..Scenario::AMBIGUOUS_STRING_LITERAL.into()
                    });
                }
                Rule::DeprecatedEqualOperator => {
                    self.add_diagnostic(Diagnostic {
                        range,
                        ..Scenario::DEPRECATED_EQUAL_OPERATOR.into()
                    });
                }
                Rule::UndocumentedCloseTag => {
                    self.add_diagnostic(Diagnostic {
                        range,
                        ..Scenario::UNDOCUMENTED_CLOSE_TAG.into()
                    });
                }
                Rule::ListBegin | Rule::SwitchBegin => {
                    ctx.scope.push(rule);
                }
                Rule::ListClose | Rule::SwitchClose => {
                    ctx.scope.pop();
                }
                Rule::BreakStmt => match ctx.scope.last() {
                    Some(scope_rule) => {
                        if *scope_rule == Rule::ListBegin {
                            self.add_diagnostic(Diagnostic {
                                range,
                                ..Scenario::DEPRECATED_LIST_BREAK.into()
                            })
                        }
                    }
                    None => self.add_diagnostic(Diagnostic {
                        range,
                        ..Scenario::UNEXPECTED_BREAK_STMT.into()
                    }),
                },
                Rule::MacroNamespace => {
                    let node_text = doc.get_ranged_text(node.start_byte()..node.end_byte());
                    let macro_call = Symbol {
                        rule,
                        start_byte: node.start_byte(),
                        end_byte: node.end_byte(),
                        range,
                    };
                    ctx.macro_call_map
                        .entry(node_text)
                        .and_modify(|macro_calls| macro_calls.push(macro_call))
                        .or_insert(vec![macro_call]);
                }
                _ => {}
            }
        }
    }
}

impl DiagnosticFeature for Reactor {
    async fn on_diagnostic(
        &self,
        _: DocumentDiagnosticParams,
    ) -> jsonrpc::Result<DocumentDiagnosticReportResult> {
        // TODO: Unchanged support
        Ok(DocumentDiagnosticReportResult::Report(
            DocumentDiagnosticReport::Full(self.get_analysis().get_analyzed_full_diagnostics()),
        ))
    }
}
