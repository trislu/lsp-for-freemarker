// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

import XCTest
import SwiftTreeSitter
import TreeSitterFreemarker

final class TreeSitterFreemarkerTests: XCTestCase {
    func testCanLoadGrammar() throws {
        let parser = Parser()
        let language = Language(language: tree_sitter_freemarker())
        XCTAssertNoThrow(try parser.setLanguage(language),
                         "Error loading Freemarker grammar")
    }
}
