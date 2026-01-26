// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

use std::collections::HashMap;

use crate::symbol::MacroNamespace;
use tower_lsp_server::ls_types::{
    FoldingRange, RelatedFullDocumentDiagnosticReport, SemanticToken, Uri,
};
use tree_sitter::Node;

#[derive(Clone, Default)]
pub struct Analysis {
    pub tokens: Vec<SemanticToken>,
    pub diagnostic: RelatedFullDocumentDiagnosticReport,
    pub folding: Vec<FoldingRange>,
    pub macro_map: HashMap<String, MacroNamespace>,
    pub valid_imports: HashMap<String, Uri>,
}

pub trait AstAnalyzer {
    fn analyze_node(&mut self, node: &Node, source: &str, analysis: &mut Analysis);
}

pub fn do_analyze(
    node: &Node,
    source: &str,
    analyzers: &mut Vec<&mut dyn AstAnalyzer>,
    analysis: &mut Analysis,
) {
    for analyzer in analyzers.iter_mut() {
        analyzer.analyze_node(node, source, analysis);
    }
    // Perform a DFS traversing
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            do_analyze(&child, source, analyzers, analysis)
        }
    }
}
