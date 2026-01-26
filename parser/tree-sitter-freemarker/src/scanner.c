/*
 * Copyright 2025-2026 Nokia
 * Licensed under the BSD 3-Clause License.
 * SPDX-License-Identifier: BSD-3-Clause
 */

// scanner.c
#include "tree_sitter/alloc.h"
#include "tree_sitter/parser.h"

#include <ctype.h>  // For isspace
#include <stddef.h> // For size_t
#include <stdint.h> // For uint64_t
#include <string.h> // For memcpy & memset

// These must match the order in `externals` array in grammar.js
enum TokenType {
    _DIRECTIVE_CLOSE_TAG,         // 0
    _GREATER_THAN_OPERATOR,       // 1
    _OPEN_PAREN,                  // 2
    _CLOSE_PAREN,                 // 3
    _GREATER_THAN_EQUAL_OPERATOR, // 4
    DEPRECATED_EQUAL_OPERATOR,    // 5
    _EQUAL_OPERATOR,              // 6
    COMMENT,                      // 7
};

typedef struct {
    TSLexer *lex;
    const bool *opt;
    // If the parenthesis_depth overflows, that means the file contains at least
    // UINT64_MAX characters, which is approximately 16 Exabytes(EB).
    uint64_t parenthesis_depth;
    bool in_comment;
    uint8_t rsv[3];
} Context;

static const size_t context_size = sizeof(Context);

static inline void context_attach(Context *ctx, TSLexer *lex, const bool *opt) {
    ctx->lex = lex;
    ctx->opt = opt;
}

// --- Standard Tree-sitter Scanner Functions ---

// Create a new scanner instance
void *tree_sitter_freemarker_external_scanner_create() {
    Context *ctx = (Context *)ts_calloc(1, context_size);
    return ctx;
}

// Destroy the scanner instance
void tree_sitter_freemarker_external_scanner_destroy(void *payload) {
    Context *ctx = (Context *)payload;
    ts_free(ctx);
}

// Serialize the scanner's state (for incremental parsing)
unsigned tree_sitter_freemarker_external_scanner_serialize(void *payload,
                                                           char *buffer) {
    memcpy(buffer, payload, context_size);
    return context_size;
}

// Deserialize the scanner's state
void tree_sitter_freemarker_external_scanner_deserialize(void *payload,
                                                         const char *buffer,
                                                         unsigned length) {

    if (length == context_size) {
        // restore scanner state
        memcpy(payload, buffer, context_size);
    } else {
        // Reset if no state is provided (e.g., initial parse)
        memset(payload, 0, context_size);
    }
}

// --- Helper functions of lexer ---

static inline int32_t lex_nextchar(TSLexer *l) {
    // The current next character in the input stream, represented as a 32-bit
    // unicode code point. see also
    // https://tree-sitter.github.io/tree-sitter/creating-parsers/4-external-scanners.html#scan
    return l->lookahead;
}

static inline void lex_skip(TSLexer *l) {
    // passing "true" to ignore whitespace, see also
    // https://tree-sitter.github.io/tree-sitter/creating-parsers/4-external-scanners.html#scan
    l->advance(l, true);
}

static inline bool lex_advance(TSLexer *l) {
    l->advance(l, false);
    return true;
}

static inline bool lex_eof(TSLexer *l) {
    // A function for determining whether the lexer is at the end of the file.
    // The value of lookahead will be 0 at the end of a file, but this function
    // should be used instead of checking for that value because the 0 or "NUL"
    // value is also a valid character that could be present in the file being
    // parsed.
    return l->eof(l);
}

static inline void lex_emit(TSLexer *l, uint16_t token) {
    // emit the recognized token from the TokenType enum.
    l->result_symbol = token;
}

static inline bool lex_matchc(TSLexer *l, char c) {
    return !lex_eof(l) && (lex_nextchar(l) == c) && lex_advance(l);
}

static bool lex_matchs(TSLexer *l, const char *str) {
    for (size_t i = 0; i < strlen(str); i++) {
        if (!lex_matchc(l, str[i])) {
            return false;
        }
    }
    return true;
}

static bool try_emit(Context *ctx, int token) {
    TSLexer *lex = ctx->lex;
    const bool *opt = ctx->opt;
    if (!opt[token]) {
        return false;
    }
    switch (token) {
    case _OPEN_PAREN:
        ctx->parenthesis_depth++;
        lex_advance(lex); // shift '('
        break;
    case _CLOSE_PAREN:
        if (ctx->parenthesis_depth > 0) {
            ctx->parenthesis_depth--;
        }
        lex_advance(lex); // shift ')'
        break;
    }
    lex_emit(lex, token);
    return true;
}

static bool scan_comment(TSLexer *lexer) {
    if (!lex_matchs(lexer, "--")) {
        return false;
    }
    while (!lex_eof(lexer)) {
        if (lex_nextchar(lexer) == '-') {
            lex_advance(lexer);
            if (lex_nextchar(lexer) == '-') {
                lex_advance(lexer);
                if (lex_nextchar(lexer) == '>') {
                    lex_advance(lexer);
                    lexer->result_symbol = COMMENT;
                    return true;
                }
            }
        } else {
            lex_advance(lexer);
        }
    }
    return false;
}

static bool ftl_spec(Context *ctx) {
    TSLexer *lex = ctx->lex;
    // shift '<'
    lex_advance(lex);
    switch (lex_nextchar(lex)) {
    case '#':
        lex_advance(lex);
        if (lex_nextchar(lex) == '-') {
            return scan_comment(lex);
        }
        break;
    }
    return false;
}

static bool recognize_token(Context *ctx, TSLexer *lex,
                            const bool *valid_symbols) {
    // attach params to context
    context_attach(ctx, lex, valid_symbols);
    // Skip any leading whitespace
    while (isspace(lex_nextchar(lex))) {
        lex_skip(lex);
    }
    if (lex_eof(lex)) {
        return false;
    }

    switch (lex_nextchar(lex)) {
    case '<':
        return ftl_spec(ctx);
    case '(':
        return try_emit(ctx, _OPEN_PAREN);
    case ')':
        return try_emit(ctx, _CLOSE_PAREN);
    case '>': {
        // Priority 1: Is the parser expecting a _DIRECTIVE_CLOSE_TAG AND we are
        // not inside parentheses?
        if (valid_symbols[_DIRECTIVE_CLOSE_TAG] &&
            ctx->parenthesis_depth == 0) {
            lex_advance(lex);
            lex_emit(lex, _DIRECTIVE_CLOSE_TAG);
            return true;
        }
        // Priority 2: Is the parser expecting a ">" or ">="?
        // This can be true whether inside or outside parentheses.
        if (valid_symbols[_GREATER_THAN_OPERATOR]) {
            lex_advance(lex); // Consume the '>' character
            if (lex_nextchar(lex) == '=') {
                lex_advance(lex); // Consume the '=' character
                lex_emit(lex, _GREATER_THAN_EQUAL_OPERATOR);
            } else {
                lex_emit(lex, _GREATER_THAN_OPERATOR);
            }
            return true;
        }
        break;
    }
    case '=': {
        // advance the 1st one
        lex_advance(lex);
        // Peek next
        if (lex_nextchar(lex) == '=' && valid_symbols[_EQUAL_OPERATOR]) {
            // advance 2nd one
            lex_advance(lex);
            // "==" is a normal equal operator
            lex_emit(lex, _EQUAL_OPERATOR);
            return true;
        }
        if (valid_symbols[DEPRECATED_EQUAL_OPERATOR]) {
            // Single '=', it's the deprecated equal
            lex_emit(lex, DEPRECATED_EQUAL_OPERATOR);
            return true;
        }
        break;
    }
    }

    // If no external token was matched, return false.
    // The main grammar will then try to match a token.
    return false;
}

// --- Main scanning logic ---
bool tree_sitter_freemarker_external_scanner_scan(void *payload, TSLexer *lex,
                                                  const bool *valid_symbols) {
    Context *ctx = (Context *)payload;
    // see if scanner can recognize an external token
    return recognize_token(ctx, lex, valid_symbols);
}