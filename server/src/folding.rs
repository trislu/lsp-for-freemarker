// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

use std::{collections::HashSet, str::FromStr};
use tower_lsp_server::{
    jsonrpc,
    ls_types::{FoldingRange, FoldingRangeParams, FoldingRangeProviderCapability},
};
use tree_sitter::Node;
use tree_sitter_freemarker::grammar::Rule;

use crate::{
    analysis::{Analysis, AstAnalyzer},
    doc::TextDocument,
    protocol::Folding,
};

pub fn folding_capability() -> FoldingRangeProviderCapability {
    FoldingRangeProviderCapability::Simple(true)
}

pub struct FoldingRangeAnalyzer {
    ranges_set: HashSet<usize>,
}

impl FoldingRangeAnalyzer {
    pub fn new() -> Self {
        FoldingRangeAnalyzer {
            ranges_set: HashSet::new(),
        }
    }
}

impl AstAnalyzer for FoldingRangeAnalyzer {
    fn analyze_node(&mut self, node: &Node, source: &str, analysis: &mut Analysis) {
        let _ = source;
        if node.is_error() || node.is_missing() {
            // not sure if it is proper
            return;
        }
        if let Ok(
            Rule::Comment
            | Rule::AssignClause
            | Rule::CaseClause
            | Rule::DefaultClause
            | Rule::ElseClause
            | Rule::FunctionClause
            | Rule::IfClause
            | Rule::ListClause
            | Rule::LocalClause
            | Rule::MacroClause
            | Rule::OnClause
            | Rule::SwitchClause,
        ) = Rule::from_str(node.kind())
        {
            // node kind with "_clause" requires indent increasing
            let id = node.id();
            if !self.ranges_set.contains(&id) {
                self.ranges_set.insert(id);
                analysis.folding.push(FoldingRange {
                    start_line: node.start_position().row as u32,
                    end_line: node.end_position().row as u32 - 1,
                    ..Default::default()
                });
            }
        }
    }
}

impl Folding for TextDocument {
    async fn on_folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> jsonrpc::Result<Option<Vec<FoldingRange>>> {
        let _ = params;
        Ok(Some(self.analyze_result.folding.clone()))
    }
}
