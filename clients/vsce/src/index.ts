/**
 * Copyright 2025-2026 Nokia
 * Licensed under the BSD 3-Clause License.
 * SPDX-License-Identifier: BSD-3-Clause
 */

import {
    commands,
    ExtensionContext,
    LogLevel,
    RelativePattern,
    TextDocument,
    Uri,
    window,
    workspace,
    WorkspaceFolder,
    WorkspaceFoldersChangeEvent,
} from "vscode";

import { Executable, LanguageClient, LanguageClientOptions, ServerOptions, Trace } from "vscode-languageclient/node";

import { bootstrap } from "./bootstrap";

const extensionId = "lsp-for-freemarker";
const extensionName = "Freemarker Language Server";
const outputChannel = window.createOutputChannel(extensionName, { log: true });

const clients: Map<string, LanguageClient> = new Map();

function freemarker_file_pattern(folder: Uri) {
    return new RelativePattern(folder, "**/*.ftl");
}

async function open_document(uri: Uri) {
    const uriMatch = (d: TextDocument) => d.uri.toString() === uri.toString();
    const doc = workspace.textDocuments.find(uriMatch);
    if (doc === undefined) {
        await workspace.openTextDocument(uri);
    }
    return uri;
}

async function start_client_for_folder(
    folder: WorkspaceFolder,
    ctx: ExtensionContext
) {
    const binary_path = await bootstrap(ctx);
    outputChannel.appendLine(`[TS] using server binary: ${binary_path}.`);

    if (!binary_path) {
        await window.showErrorMessage(
            "Not starting Freemarker Language Server as a suitable binary was not found."
        );
        return;
    }

    const root = folder.uri;

    const deleteWatcher = workspace.createFileSystemWatcher(
        freemarker_file_pattern(root),
        true, // ignoreCreateEvents
        true, // ignoreChangeEvents
        false // ignoreDeleteEvents
    );

    const createChangeWatcher = workspace.createFileSystemWatcher(
        freemarker_file_pattern(root),
        false, // ignoreCreateEvents
        false, // ignoreChangeEvents
        true // ignoreDeleteEvents
    );

    ctx.subscriptions.push(deleteWatcher);
    ctx.subscriptions.push(createChangeWatcher);

    const server_exec: Executable = {
        command: binary_path,
        options: {
            env: {
                ...process.env
            },
        }
    };

    const server_opt: ServerOptions = {
        run: server_exec,
        debug: server_exec,
    };

    let client_opt: LanguageClientOptions = {
        documentSelector: [
            { language: "ftl", pattern: `${root.fsPath}/**/*.ftl`, scheme: "file" },
        ],
        synchronize: {
            fileEvents: [
                deleteWatcher
            ]
        },
        diagnosticCollectionName: extensionName,
        workspaceFolder: folder,
        outputChannel,
        progressOnInitialization: true
    };

    const client = new LanguageClient(
        extensionId,
        extensionName,
        server_opt,
        client_opt,
    );

    ctx.subscriptions.push(createChangeWatcher.onDidCreate(open_document));
    ctx.subscriptions.push(createChangeWatcher.onDidChange(open_document));
    ctx.subscriptions.push(outputChannel.onDidChangeLogLevel((level: LogLevel) => {
        outputChannel.appendLine(`[TS] onDidChangeLogLevel: ${level}.`);
        switch (level) {
            case LogLevel.Trace:
            case LogLevel.Debug:
                client.setTrace(Trace.Verbose);
                break;
            case LogLevel.Info:
                client.setTrace(Trace.Messages);
                break;
            case LogLevel.Warning:
            case LogLevel.Error:
                client.setTrace(Trace.Compact);
                break;
            case LogLevel.Off:
            default:
                client.setTrace(Trace.Off);
                break;
        }
    }));
    await client.start();
}

function stop_client(client: LanguageClient) {
    client.diagnostics?.clear();
    return client.stop();
}

async function stop_client_for_folder(workspaceFolder: string) {
    const client = clients.get(workspaceFolder);
    if (client) {
        await stop_client(client);
    }

    clients.delete(workspaceFolder);
}

function update_clients(context: ExtensionContext) {
    return async function ({ added, removed }: WorkspaceFoldersChangeEvent) {
        for (const folder of removed) {
            await stop_client_for_folder(folder.uri.toString());
        }
        for (const folder of added) {
            await start_client_for_folder(folder, context);
        }
    };
}

export async function activate(context: ExtensionContext): Promise<void> {
    const now = new Date().toLocaleString();
    console.log(`[${now}] activate ftl-lsp extension`);

    const folders = workspace.workspaceFolders || [];

    for (const folder of folders) {
        await start_client_for_folder(folder, context);
    }

    context.subscriptions.push(
        workspace.onDidChangeWorkspaceFolders(update_clients(context))
    );

    commands.registerCommand("freemarker.restart", async () => {
        const currentFolder = workspace.workspaceFolders?.[0].uri.toString();
        if (!currentFolder) {
            await window.showInformationMessage("No Freemarker workspace is open.");
            return;
        }
        const client = clients.get(currentFolder);
        if (client) {
            const folder = client.clientOptions.workspaceFolder;
            await stop_client(client);
            if (folder) {
                await start_client_for_folder(folder, context);
            }
        }
    });
}

export async function deactivate(): Promise<void> {
    await Promise.all([...clients.values()].map(stop_client));
}
