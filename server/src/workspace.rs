use crate::{
    reactor::Reactor,
    server::{
        ActionFeature, CompletionFeature, DiagnosticFeature, FoldingFeature, FormatFeature,
        GotoFeature, HoverFeature, SemanticTokenFeature,
    },
};

use std::{collections::HashMap, str::FromStr, sync::Arc};
use tokio::sync::RwLock;
use tower_lsp_server::{
    jsonrpc,
    ls_types::{
        CodeActionOrCommand, CodeActionParams, CompletionParams, CompletionResponse,
        DeleteFilesParams, DidChangeTextDocumentParams, DidChangeWatchedFilesParams,
        DidOpenTextDocumentParams, DocumentDiagnosticParams, DocumentDiagnosticReportResult,
        DocumentFormattingParams, FileChangeType, FoldingRange, FoldingRangeParams,
        GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, SemanticTokensParams,
        SemanticTokensResult, TextDocumentContentChangeEvent, TextEdit, Uri,
    },
};
use tracing::{Level, event};

#[derive(Debug)]
pub struct Workspace {
    reactors: Arc<RwLock<HashMap<Uri, Reactor>>>,
}

const GET_REACTOR_EXPECT: &str = "get reactor via url should always succeed";

impl Workspace {
    pub fn new() -> Self {
        Self {
            reactors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn open_file(&self, params: &DidOpenTextDocumentParams) {
        let url: &Uri = &params.text_document.uri;
        let version: i32 = params.text_document.version;
        event!(Level::INFO, "uri:{:?}, version:{}", url, version);
        let mut write_guard = self.reactors.write().await;
        if match write_guard.get(url) {
            Some(old_reactor) => old_reactor.version != version,
            None => true,
        } {
            let source_code = params.text_document.text.as_str();
            let reactor = Reactor::new(url, source_code, version);
            write_guard.insert(url.clone(), reactor);
        }
    }

    pub async fn on_did_change(&self, params: &DidChangeTextDocumentParams) {
        let url = &params.text_document.uri;
        let version = params.text_document.version;
        tracing::debug!("on_did_change: {:?}", url.to_file_path().unwrap());
        for change_event in &params.content_changes {
            // assume only changes
            if let Some(range) = change_event.range {
                tracing::debug!("range: {:?}", range);
                self.update_file(url, version, change_event).await;
            } else {
                tracing::debug!("full text change");
            }
        }
    }

    async fn update_file(&self, url: &Uri, version: i32, change: &TextDocumentContentChangeEvent) {
        let mut write_guard = self.reactors.write().await;
        if let Some(reactor) = write_guard.get_mut(url) {
            tracing::debug!("previous file version: {}", reactor.version);
            reactor.apply_content_change(version, change);
        }
    }

    pub async fn on_did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        let DidChangeWatchedFilesParams { changes } = params;
        // filter delete events
        let uris: Vec<_> = changes
            .into_iter()
            .filter_map(|ev| match ev.typ == FileChangeType::DELETED {
                true => Some(ev.uri),
                false => None,
            })
            .collect();
        // remove those files from the registry
        for uri in uris {
            self.reactors.write().await.remove(&uri);
        }
    }

    pub async fn on_did_delete_files(&self, params: DeleteFilesParams) {
        for file_deletion in &params.files {
            let url = Uri::from_str(&file_deletion.uri).unwrap();
            self.reactors.write().await.remove(&url);
        }
    }

    // LSP request/response
    pub async fn on_diagnostic(
        &self,
        params: DocumentDiagnosticParams,
    ) -> jsonrpc::Result<DocumentDiagnosticReportResult> {
        let url = &params.text_document.uri;
        let read_guard = self.reactors.read().await;
        let reactor = read_guard.get(url).expect(GET_REACTOR_EXPECT);
        reactor.on_diagnostic(params).await
    }

    pub async fn on_semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> jsonrpc::Result<Option<SemanticTokensResult>> {
        let url = &params.text_document.uri;
        let read_guard = self.reactors.read().await;
        let reactor = read_guard.get(url).expect(GET_REACTOR_EXPECT);
        reactor.on_semantic_tokens_full(params).await
    }

    pub async fn on_hover(&self, params: HoverParams) -> jsonrpc::Result<Option<Hover>> {
        let url = &params.text_document_position_params.text_document.uri;
        let read_guard = self.reactors.read().await;
        let reactor = read_guard.get(url).expect(GET_REACTOR_EXPECT);
        reactor.on_hover(params).await
    }

    pub async fn on_completion(
        &self,
        params: CompletionParams,
    ) -> jsonrpc::Result<Option<CompletionResponse>> {
        let url = &params.text_document_position.text_document.uri;
        let read_guard = self.reactors.read().await;
        let reactor = read_guard.get(url).expect(GET_REACTOR_EXPECT);
        reactor.on_completion(params).await
    }

    pub async fn on_goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> jsonrpc::Result<Option<GotoDefinitionResponse>> {
        let url = &params.text_document_position_params.text_document.uri;
        let read_guard = self.reactors.read().await;
        let reactor = read_guard.get(url).expect(GET_REACTOR_EXPECT);
        reactor.on_goto_definition(params).await
    }

    pub async fn on_formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> jsonrpc::Result<Option<Vec<TextEdit>>> {
        let url = &params.text_document.uri;
        let read_guard = self.reactors.read().await;
        let reactor = read_guard.get(url).expect(GET_REACTOR_EXPECT);
        reactor.on_formatting(params).await
    }

    pub async fn on_folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> jsonrpc::Result<Option<Vec<FoldingRange>>> {
        let url = &params.text_document.uri;
        let read_guard = self.reactors.read().await;
        let reactor = read_guard.get(url).expect(GET_REACTOR_EXPECT);
        reactor.on_folding_range(params).await
    }

    pub async fn on_code_action(
        &self,
        params: CodeActionParams,
    ) -> jsonrpc::Result<Option<Vec<CodeActionOrCommand>>> {
        let url = &params.text_document.uri;
        let read_guard = self.reactors.read().await;
        let reactor = read_guard.get(url).expect(GET_REACTOR_EXPECT);
        reactor.on_code_action(params).await
    }
}
