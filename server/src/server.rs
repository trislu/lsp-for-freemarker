// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

use std::{fmt::Display, sync::Arc};

use tokio::sync::RwLock;
use tower_lsp_server::{
    Client, LanguageServer, jsonrpc,
    ls_types::{
        CodeActionOrCommand, CodeActionParams, CompletionItem, CompletionParams,
        CompletionResponse, DeleteFilesParams, DidChangeTextDocumentParams,
        DidChangeWatchedFilesParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
        DocumentDiagnosticParams, DocumentDiagnosticReportResult, DocumentFormattingParams,
        FoldingRange, FoldingRangeParams, GotoDefinitionParams, GotoDefinitionResponse, Hover,
        HoverParams, InitializeParams, InitializeResult, InitializedParams, MessageType,
        SemanticTokensParams, SemanticTokensResult, TextEdit,
    },
};
use tracing::{self, instrument};

use crate::workspace::Workspace;

#[derive(Debug)]
pub struct Server {
    pub client: Client,
    pub(crate) root_path: Arc<RwLock<String>>,
    pub(crate) workspace: Workspace,
}

impl Server {
    pub const NAME: &str = "Freemarker Language Server";
    pub const CODE_NAME: &str = "lsp-for-freemarker";

    pub fn new(client: Client) -> Self {
        Self {
            client,
            root_path: Arc::new(RwLock::new(String::new())),
            workspace: Workspace::new(),
        }
    }

    // TODO: add log_debug support, currently MessageType only support INFO/WARNING/ERROR
    // see also https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#messageType
    pub async fn log_info<M: Display>(&self, message: M) {
        self.client.log_message(MessageType::INFO, message).await;
    }
}

pub trait Initializer {
    async fn on_initialize(&self, params: InitializeParams) -> InitializeResult;
}

//#[tower_lsp_server::async_trait]
impl LanguageServer for Server {
    async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        return Ok(self.on_initialize(params).await);
    }

    #[instrument(skip_all)]
    async fn initialized(&self, _: InitializedParams) {
        self.log_info("language server initialized").await;
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        self.log_info("language server shutting down :)").await;
        Ok(())
    }

    #[instrument(skip_all)]
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.log_info(format!("did_open: {:?}", params.text_document.uri))
            .await;
        self.workspace.open_file(&params).await;
    }

    #[instrument(skip_all)]
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.log_info(format!("did_change: {:?}", params.text_document.uri))
            .await;
        self.workspace.on_did_change(&params).await;
    }

    #[instrument(skip_all)]
    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let url = &params.text_document.uri;
        tracing::debug!("did_close: {:?}", url.to_file_path().unwrap());
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        self.workspace.on_did_change_watched_files(params).await;
    }

    async fn did_delete_files(&self, params: DeleteFilesParams) {
        self.workspace.on_did_delete_files(params).await;
    }

    // LSP request/response
    #[instrument(skip_all)]
    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> jsonrpc::Result<DocumentDiagnosticReportResult> {
        self.workspace.on_diagnostic(params).await
    }

    #[instrument(skip_all)]
    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> jsonrpc::Result<Option<SemanticTokensResult>> {
        self.log_info(format!(
            "semantic_tokens_full: {:?}",
            params.text_document.uri
        ))
        .await;
        self.workspace.on_semantic_tokens_full(params).await
    }

    async fn hover(&self, params: HoverParams) -> jsonrpc::Result<Option<Hover>> {
        self.workspace.on_hover(params).await
    }

    async fn completion(
        &self,
        params: CompletionParams,
    ) -> jsonrpc::Result<Option<CompletionResponse>> {
        self.workspace.on_completion(params).await
    }

    #[instrument(skip_all)]
    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> jsonrpc::Result<Option<GotoDefinitionResponse>> {
        self.workspace.on_goto_definition(params).await
    }

    #[instrument(skip_all)]
    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> jsonrpc::Result<Option<Vec<TextEdit>>> {
        self.workspace.on_formatting(params).await
    }

    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> jsonrpc::Result<Option<Vec<FoldingRange>>> {
        self.workspace.on_folding_range(params).await
    }

    #[instrument(skip_all)]
    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> jsonrpc::Result<Option<Vec<CodeActionOrCommand>>> {
        self.workspace.on_code_action(params).await
    }
}

// LSP features
pub trait ActionFeature {
    async fn on_code_action(
        &self,
        params: CodeActionParams,
    ) -> jsonrpc::Result<Option<Vec<CodeActionOrCommand>>>;
}

pub trait CompletionFeature {
    async fn on_completion(
        &self,
        params: CompletionParams,
    ) -> jsonrpc::Result<Option<CompletionResponse>>;

    fn list_macro_definitions(&self) -> Vec<CompletionItem>;
}

pub trait DiagnosticFeature {
    async fn on_diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> jsonrpc::Result<DocumentDiagnosticReportResult>;
}

pub trait FoldingFeature {
    async fn on_folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> jsonrpc::Result<Option<Vec<FoldingRange>>>;
}

pub trait FormatFeature {
    async fn on_formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> jsonrpc::Result<Option<Vec<TextEdit>>>;
}

pub trait GotoFeature {
    async fn on_goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> jsonrpc::Result<Option<GotoDefinitionResponse>>;
}

pub trait HoverFeature {
    //fn on_node(&self, position: Position) -> Option<Node<'_>>;
    async fn on_hover(&self, params: HoverParams) -> jsonrpc::Result<Option<Hover>>;
}

pub trait SemanticTokenFeature {
    async fn on_semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> jsonrpc::Result<Option<SemanticTokensResult>>;
}
