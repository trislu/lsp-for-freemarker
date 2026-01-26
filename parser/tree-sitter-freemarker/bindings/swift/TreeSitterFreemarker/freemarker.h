/*
 * Copyright 2025-2026 Nokia
 * Licensed under the BSD 3-Clause License.
 * SPDX-License-Identifier: BSD-3-Clause
 */

#ifndef TREE_SITTER_FREEMARKER_H_
#define TREE_SITTER_FREEMARKER_H_

typedef struct TSLanguage TSLanguage;

#ifdef __cplusplus
extern "C" {
#endif

const TSLanguage *tree_sitter_freemarker(void);

#ifdef __cplusplus
}
#endif

#endif // TREE_SITTER_FREEMARKER_H_
