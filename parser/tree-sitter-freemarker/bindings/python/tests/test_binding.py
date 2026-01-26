# Copyright 2025-2026 Nokia
# Licensed under the BSD 3-Clause License.
# SPDX-License-Identifier: BSD-3-Clause

from unittest import TestCase

import tree_sitter, tree_sitter_freemarker


class TestLanguage(TestCase):
    def test_can_load_grammar(self):
        try:
            tree_sitter.Language(tree_sitter_freemarker.language())
        except Exception:
            self.fail("Error loading Freemarker grammar")
