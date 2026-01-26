/**
 * Copyright 2025-2026 Nokia
 * Licensed under the BSD 3-Clause License.
 * SPDX-License-Identifier: BSD-3-Clause
 */

import { ExtensionContext, Uri, window, workspace } from "vscode";
import * as os from "os";

export async function bootstrap(
    context: ExtensionContext
): Promise<string> {
    const path = await get_server(context);
    if (!path) {
        throw new Error("freemarker-language-server is not available.");
    }
    const now = new Date().toLocaleString();
    console.log(`[${now}] Using server binary at ${path}`);
    // TODO: check validity
    return path;
}

async function get_server(
    context: ExtensionContext
): Promise<string | undefined> {
    // check if the server path is configured explicitly
    const explicitPath = process.env["__FTL_LSP_SERVER_DEBUG"];
    if (explicitPath) {
        if (explicitPath.startsWith("~/")) {
            return os.homedir() + explicitPath.slice("~".length);
        }
        return explicitPath;
    }

    const ext = process.platform === "win32" ? ".exe" : "";
    const bundled = Uri.joinPath(context.extensionUri, "resources", "bin", `ftl-lsp-server${ext}`);
    const bundledExists = await fileExists(bundled);

    if (!bundledExists) {
        await window.showErrorMessage(
            "Unfortunately we don't ship binaries for your platform yet. " +
            "Please build and run the server manually from the source code. " +
            "Or, please create an issue on repository."
        );
        return;
    }

    return bundled.fsPath;
}

async function fileExists(uri: Uri) {
    return await workspace.fs.stat(uri).then(
        () => true,
        () => false,
    );
}

// TODO: check availability of the server binary
