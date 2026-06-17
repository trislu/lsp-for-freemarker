// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

#![deny(clippy::print_stdout)]
#![deny(clippy::print_stderr)]

use tower_lsp_server::LspService;

mod action;
mod analysis;
mod client;
mod completion;
mod diagnosis;
mod doc;
mod folding;
mod format;
mod goto;
mod hover;
mod init;
mod parser;
mod reactor;
mod server;
mod symbol;
mod tokenizer;
mod utils;
mod workspace;

#[tokio::main]
async fn main() {
    // TODO: support other commands (e.g. `--version`, `--log`)
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(server::Server::new);
    tower_lsp_server::Server::new(stdin, stdout, socket)
        .serve(service)
        .await;
}
