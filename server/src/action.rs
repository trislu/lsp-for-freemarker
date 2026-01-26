// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

use std::{collections::HashMap, str::FromStr};
use tower_lsp_server::{
    jsonrpc::Result as JsonRpcResult,
    ls_types::{
        CodeAction, CodeActionKind, CodeActionOptions, CodeActionOrCommand, CodeActionParams,
        CodeActionProviderCapability, Diagnostic, NumberOrString, TextEdit, Uri,
        WorkDoneProgressOptions, WorkspaceEdit,
    },
};
use tree_sitter_freemarker::grammar::Rule;

use crate::{doc::TextDocument, protocol::Action};

#[allow(clippy::mutable_key_type)]
fn create_fix_warning_action(
    code: &String,
    uri: &Uri,
    diagnostic: Diagnostic,
) -> Option<CodeActionOrCommand> {
    let mut changes: HashMap<Uri, Vec<TextEdit>> = HashMap::new();

    // The TextEdit describes replacing the diagnostic's range with the correct text
    let text_edit = TextEdit {
        range: diagnostic.range,
        new_text: match Rule::from_str(code.as_str()) {
            Ok(rule) => match rule {
                Rule::DeprecatedEqualOperator => "==".to_string(),
                Rule::UndocumentedCloseTag => ">".to_string(),
                _ => return None,
            },
            Err(_) => return None,
        },
    };

    changes.insert(uri.clone(), vec![text_edit]);

    Some(CodeActionOrCommand::CodeAction(CodeAction {
        title: format!("fix warning: {}", code),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic]), // Link this action to the diagnostic it fixes
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        command: None,
        is_preferred: Some(true), // Suggest this as the primary fix
        data: None,
        disabled: None,
    }))
}

pub fn code_action_capability() -> CodeActionProviderCapability {
    CodeActionProviderCapability::Options(CodeActionOptions {
        code_action_kinds: Some(vec![CodeActionKind::QUICKFIX]),
        work_done_progress_options: WorkDoneProgressOptions::default(),
        resolve_provider: None,
        //FIXME: Use Default here once https://github.com/gluon-lang/lsp-types/issues/260 is resolved.
        // ..Default::default()
    })
}

impl Action for TextDocument {
    async fn on_code_action(
        &self,
        params: CodeActionParams,
    ) -> JsonRpcResult<Option<Vec<CodeActionOrCommand>>> {
        let mut actions: Vec<CodeActionOrCommand> = Vec::new();
        for diagnostic in params.context.diagnostics {
            if let Some(NumberOrString::String(code)) = &diagnostic.code {
                // string codes
                if let Some(fix_action) =
                    create_fix_warning_action(code, &params.text_document.uri, diagnostic.clone())
                {
                    // Create a CodeAction for this specific diagnostic
                    actions.push(fix_action);
                }
            }
        }
        Ok(Some(actions))
    }
}
