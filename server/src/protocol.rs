// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

use tower_lsp_server::{
    LanguageServer, jsonrpc,
    ls_types::{
        CodeActionOrCommand, CodeActionParams, CompletionParams, CompletionResponse,
        DeleteFilesParams, DidChangeTextDocumentParams, DidChangeWatchedFilesParams,
        DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentDiagnosticParams,
        DocumentDiagnosticReportResult, DocumentFormattingParams, FoldingRange, FoldingRangeParams,
        GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, InitializeParams,
        InitializeResult, InitializedParams, SemanticTokensParams, SemanticTokensResult, TextEdit,
    },
};

use crate::server::Server;

//#[tower_lsp_server::async_trait]
impl LanguageServer for Server {
    async fn initialize(&self, params: InitializeParams) -> jsonrpc::Result<InitializeResult> {
        return Ok(self.on_initialize(params).await);
    }

    async fn initialized(&self, _: InitializedParams) {
        self.log_info("language server initialized").await;
    }

    async fn shutdown(&self) -> jsonrpc::Result<()> {
        self.log_info("language server shutting down :)").await;
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.on_did_open(&params).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.on_did_change(&params).await;
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.on_did_close(&params).await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        self.on_did_change_watched_files(params).await;
    }

    async fn did_delete_files(&self, params: DeleteFilesParams) {
        self.on_did_delete_files(params).await;
    }

    // LSP request/response
    async fn diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> jsonrpc::Result<DocumentDiagnosticReportResult> {
        let url = &params.text_document.uri;
        let doc_map = self.doc_map.read().await;
        let doc = doc_map
            .get(url)
            .expect("get document via url should always succeed");
        doc.on_diagnostic(params).await
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> jsonrpc::Result<Option<SemanticTokensResult>> {
        let url = &params.text_document.uri;
        let doc_map = self.doc_map.read().await;
        let doc = doc_map
            .get(url)
            .expect("get document via url should always succeed");
        doc.on_semantic_tokens_full(params).await
    }

    #[tracing::instrument(skip_all)]
    async fn hover(&self, params: HoverParams) -> jsonrpc::Result<Option<Hover>> {
        let url = &params.text_document_position_params.text_document.uri;
        let doc_map = self.doc_map.read().await;
        let doc = doc_map
            .get(url)
            .expect("get document via url should always succeed");
        doc.on_hover(params).await
    }

    #[tracing::instrument(skip_all)]
    async fn completion(
        &self,
        params: CompletionParams,
    ) -> jsonrpc::Result<Option<CompletionResponse>> {
        let url = &params.text_document_position.text_document.uri;
        let doc_map = self.doc_map.read().await;
        let doc = doc_map
            .get(url)
            .expect("get document via url should always succeed");
        doc.on_completion(params).await
    }

    #[tracing::instrument(skip(self))]
    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> jsonrpc::Result<Option<GotoDefinitionResponse>> {
        let doc_map = self.doc_map.read().await;
        let url = &params.text_document_position_params.text_document.uri;
        let doc = doc_map
            .get(url)
            .expect("get document via url should always succeed");
        doc.on_goto_definition(params).await
    }

    #[tracing::instrument(skip(self))]
    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> jsonrpc::Result<Option<Vec<TextEdit>>> {
        let url = &params.text_document.uri;
        let doc_map = self.doc_map.read().await;
        let doc = doc_map
            .get(url)
            .expect("get document via url should always succeed");
        doc.on_formatting(params).await
    }

    #[tracing::instrument(skip_all)]
    async fn folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> jsonrpc::Result<Option<Vec<FoldingRange>>> {
        let url = &params.text_document.uri;
        let doc_map = self.doc_map.read().await;
        let doc = doc_map
            .get(url)
            .expect("get document via url should always succeed");
        doc.on_folding_range(params).await
    }

    #[tracing::instrument(skip_all)]
    async fn code_action(
        &self,
        params: CodeActionParams,
    ) -> jsonrpc::Result<Option<Vec<CodeActionOrCommand>>> {
        let url = &params.text_document.uri;
        let doc_map = self.doc_map.read().await;
        let doc = doc_map
            .get(url)
            .expect("get document via url should always succeed");
        doc.on_code_action(params).await
    }
}

// LSP features
pub trait Initializer {
    async fn on_initialize(&self, params: InitializeParams) -> InitializeResult;
}

pub trait Hovering {
    async fn on_hover(&self, params: HoverParams) -> jsonrpc::Result<Option<Hover>>;
}

pub trait Completion {
    async fn on_completion(
        &self,
        params: CompletionParams,
    ) -> jsonrpc::Result<Option<CompletionResponse>>;
}

pub trait Goto {
    async fn on_goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> jsonrpc::Result<Option<GotoDefinitionResponse>>;
}

pub trait Formatter {
    async fn on_formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> jsonrpc::Result<Option<Vec<TextEdit>>>;
}

pub trait Action {
    async fn on_code_action(
        &self,
        params: CodeActionParams,
    ) -> jsonrpc::Result<Option<Vec<CodeActionOrCommand>>>;
}

pub trait Folding {
    async fn on_folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> jsonrpc::Result<Option<Vec<FoldingRange>>>;
}

pub trait Diagnose {
    async fn on_diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> jsonrpc::Result<DocumentDiagnosticReportResult>;
}

pub trait Tokenizer {
    async fn on_semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> jsonrpc::Result<Option<SemanticTokensResult>>;
}
