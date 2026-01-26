// Copyright 2025-2026 Nokia
// Licensed under the BSD 3-Clause License.
// SPDX-License-Identifier: BSD-3-Clause

package tree_sitter_freemarker_test

import (
	"testing"

	tree_sitter "github.com/tree-sitter/go-tree-sitter"
	tree_sitter_freemarker "github.com/nokia/lsp-for-freemarker.git/parser/tree-sitter-freemarker/bindings/go"
)

func TestCanLoadGrammar(t *testing.T) {
	language := tree_sitter.NewLanguage(tree_sitter_freemarker.Language())
	if language == nil {
		t.Errorf("Error loading Freemarker grammar")
	}
}
