# tree-sitter-freemarker
> A [_tree-sitter_](https://tree-sitter.github.io/tree-sitter/index.html) parser library for [`Freemarker Template Language`](https://freemarker.apache.org/docs/ref.html)

## Project Status

**tree-sitter-freemarker** is used by [lsp-for-freemarker](https://github.com/nokia/lsp-for-freemarker), both are still in development and not all features have the same good test coverage.

### Quick Q&A

> Q: Will this library be moved into another repo?<br>
> A: When necessary.

> Q: How many language bindings does this library provide?<br>
> A: See `bindings/`, but only the `rust` is maintained for now.

## Development
###  Prerequisites
+ __C__ compiler: supports `C11`
+ __RUST__ toolchain: `1.90.0`
+ [tree-sitter-cli](https://tree-sitter.github.io/tree-sitter/creating-parsers/1-getting-started.html#installation) version `0.25.10` 
+ [_tree-sitter_](https://tree-sitter.github.io/tree-sitter/index.html) _--abi=14_

### Commands
+ `tree-sitter build`: compile the parser
+ `cargo build`: build the _rust_ binding
+ `cargo test`: run test cases

### Parsing
```rust
let code = "<#ftl hello=\"world\">";
let mut parser = tree_sitter::Parser::new();
let lang = tree_sitter_freemarker::LANGUAGE;
parser
    .set_language(&lang.into())
    .expect("Error loading Freemarker parser");
let ast = parser.parse(code, None).unwrap();
assert!(!ast.root_node().has_error());
```

## License

This project is licensed under the BSD-3-Clause license - see the [LICENSE](https://github.com/nokia/lsp-for-freemarker/parser/tree-sitter-freemarker/blob/master/LICENSE).
