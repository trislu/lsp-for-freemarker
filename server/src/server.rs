// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

use std::fmt::Display;
use std::str::FromStr;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

use tower_lsp_server::ls_types::{
    DeleteFilesParams, DidChangeWatchedFilesParams, FileChangeType, FileEvent,
};
use tower_lsp_server::{
    Client,
    ls_types::{
        DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
        MessageType, TextDocumentContentChangeEvent, Uri,
    },
};

use crate::doc::{PositionEncodingKind, TextDocument};

pub struct Server {
    pub client: Client,
    pub(crate) doc_map: Arc<RwLock<HashMap<Uri, TextDocument>>>,
    pub(crate) root_path: Arc<RwLock<String>>,
}

impl Server {
    pub const NAME: &str = "Freemarker Language Server";
    pub const CODE_NAME: &str = "lsp-for-freemarker";

    pub fn new(client: Client) -> Self {
        Self {
            client,
            doc_map: Arc::new(RwLock::new(HashMap::new())),
            root_path: Arc::new(RwLock::new(String::new())),
        }
    }

    // TODO: add log_debug support, currently MessageType only support INFO/WARNING/ERROR
    // see also https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#messageType
    pub async fn log_info<M: Display>(&self, message: M) {
        self.client.log_message(MessageType::INFO, message).await;
    }

    #[tracing::instrument(skip_all)]
    pub async fn update_file(
        &self,
        url: &Uri,
        version: i32,
        change: &TextDocumentContentChangeEvent,
    ) {
        let mut wr = self.doc_map.write().await;
        if let Some(doc) = wr.get_mut(url) {
            tracing::debug!("previous file version: {}", doc.version);
            doc.apply_content_change(version, change, PositionEncodingKind::UTF8)
                .unwrap();
        }
    }

    pub async fn on_did_open(&self, params: &DidOpenTextDocumentParams) {
        let url: &Uri = &params.text_document.uri;
        let version: i32 = params.text_document.version;
        let source_code = params.text_document.text.as_str();
        let mut wr = self.doc_map.write().await;
        if match wr.get(url) {
            Some(exist) => exist.version != version,
            None => true,
        } {
            let doc = TextDocument::new(url, source_code, version);
            wr.insert(url.clone(), doc);
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn on_did_change(&self, params: &DidChangeTextDocumentParams) {
        let url = &params.text_document.uri;
        let version = params.text_document.version;
        tracing::debug!("on_did_change: {:?}", url.to_file_path().unwrap());
        for c in &params.content_changes {
            // assume only changes
            if let Some(range) = c.range {
                tracing::debug!("range: {:?}", range);
                self.update_file(url, version, c).await;
            } else {
                tracing::debug!("full text change");
            }
        }
    }

    #[tracing::instrument(skip_all)]
    pub async fn on_did_close(&self, params: &DidCloseTextDocumentParams) {
        let url = &params.text_document.uri;
        tracing::debug!("on_did_close: {:?}", url.to_file_path().unwrap());
    }

    pub async fn on_did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        let DidChangeWatchedFilesParams { changes } = params;
        let uris: Vec<_> = changes
            .into_iter()
            .map(|FileEvent { uri, typ }| {
                assert_eq!(typ, FileChangeType::DELETED);
                uri
            })
            .collect();
        for uri in uris {
            self.doc_map.write().await.remove(&uri);
        }
    }

    pub async fn on_did_delete_files(&self, params: DeleteFilesParams) {
        self.log_info("on_did_delete_files:").await;
        for f in &params.files {
            let url = Uri::from_str(&f.uri).unwrap();
            self.doc_map.write().await.remove(&url);
        }
    }
}
