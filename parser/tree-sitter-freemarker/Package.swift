// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

// swift-tools-version:5.3
import PackageDescription

let package = Package(
    name: "TreeSitterFreemarker",
    products: [
        .library(name: "TreeSitterFreemarker", targets: ["TreeSitterFreemarker"]),
    ],
    dependencies: [
        .package(url: "https://github.com/ChimeHQ/SwiftTreeSitter", from: "0.8.0"),
    ],
    targets: [
        .target(
            name: "TreeSitterFreemarker",
            dependencies: [],
            path: ".",
            sources: [
                "src/parser.c",
                // NOTE: if your language has an external scanner, add it here.
            ],
            resources: [
                .copy("queries")
            ],
            publicHeadersPath: "bindings/swift",
            cSettings: [.headerSearchPath("src")]
        ),
        .testTarget(
            name: "TreeSitterFreemarkerTests",
            dependencies: [
                "SwiftTreeSitter",
                "TreeSitterFreemarker",
            ],
            path: "bindings/swift/TreeSitterFreemarkerTests"
        )
    ],
    cLanguageStandard: .c11
)
