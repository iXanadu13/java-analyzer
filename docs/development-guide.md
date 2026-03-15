# Development Guide

This document covers how to build, test, and extend java-analyzer.

---

## Prerequisites

- **Rust** (edition 2024, see `rust-toolchain.toml` for the exact pinned toolchain)
- **Java** (JDK 11+ recommended; needed for decompiler integration tests)
- **Gradle** (optional; only needed for Gradle integration testing)
- **Nix** (optional; `flake.nix` provides a fully reproducible dev shell via `direnv`)

### Quick start without Nix

```sh
# Install the correct Rust toolchain
rustup show   # reads rust-toolchain.toml automatically

# Build
cargo build

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run
```

### Quick start with Nix + direnv

```sh
direnv allow   # activates the flake dev shell automatically
cargo build
```

---

## Project Structure Quick Reference

| Path | What lives here |
|---|---|
| `src/main.rs` | Binary entry point |
| `src/lib.rs` | Crate root; module declarations |
| `src/lsp/` | LSP server, request handlers, capabilities, config |
| `src/language/java/` | All Java-specific logic |
| `src/semantic/` | Type system, type resolver, cursor context |
| `src/index/` | Class metadata, indexing, index hierarchy |
| `src/completion/` | Completion engine, providers, fuzzy, post-processing |
| `src/build_integration/` | Gradle model extraction and workspace reload |
| `src/workspace/` | Document store and analysis context routing |
| `src/decompiler/` | Decompiler backends and disk cache |
| `src/jvm/` | Low-level JVM descriptor / constant-pool utilities |
| `editors/code/` | VS Code extension |
| `docs/` | Design and reference documentation |

---

## Running Tests

The project uses two testing approaches:

### Unit and integration tests

```sh
cargo test                        # all tests
cargo test --lib                  # lib tests only
cargo test <module_path>          # e.g. cargo test index::tests
cargo test -- --nocapture         # show println! output
```

### Snapshot tests (insta)

Snapshot tests live in `src/**/snapshots/` directories (`.snap` files). They capture structured outputs — ASTs, completion lists, type inference results — and fail when output changes unexpectedly.

```sh
# Run and review changed snapshots interactively
cargo insta test
cargo insta review

# Accept all pending snapshot updates
cargo insta accept
```

Always review snapshot diffs carefully before accepting — they encode semantic correctness.

### CI

GitHub Actions runs `cargo test` and `cargo clippy` on push/PR (see `.github/workflows/rust-ci.yaml`).

---

## LSP Configuration

The server reads configuration from the editor via `workspace/didChangeConfiguration`. The config schema is defined in `src/lsp/config.rs`:

```jsonc
{
  // Path to the JDK home directory (JAVA_HOME equivalent)
  // If omitted, JDK classes are not indexed
  "jdkPath": "/usr/lib/jvm/java-21",

  // Path to the decompiler JAR (Vineflower or CFR)
  "decompilerPath": "/path/to/vineflower.jar",

  // Which decompiler backend to use: "vineflower" (default) or "cfr"
  "decompilerBackend": "vineflower"
}
```

The VS Code extension (`editors/code/`) passes these settings to the server on startup and configuration change.

---

## Adding a New Completion Provider

1. Create a new file under `src/language/java/completion/providers/myprovider.rs`.
2. Implement `CompletionProvider`:

```rust
use crate::completion::provider::{CompletionProvider, ProviderCompletionResult, ProviderSearchSpace};
use crate::index::IndexView;
use crate::language::Language;
use crate::index::IndexScope;
use crate::semantic::SemanticContext;

pub struct MyProvider;

impl CompletionProvider for MyProvider {
    fn search_space(&self) -> ProviderSearchSpace {
        ProviderSearchSpace::Narrow  // or Broad for fuzzy providers
    }

    fn provide(
        &self,
        scope: IndexScope,
        ctx: &SemanticContext,
        lang: &dyn Language,
        index: &IndexView,
    ) -> ProviderCompletionResult {
        // Return Vec<CompletionCandidate>
        ProviderCompletionResult::empty()
    }
}
```

3. Export it from `src/language/java/completion/providers.rs`.
4. Add `&MyProvider` to the `JAVA_COMPLETION_PROVIDERS` static array in `src/language/java.rs`.

Providers are stateless. All request-scoped state is in `SemanticContext`.

---

## Adding Support for a New Java Feature

Most new Java syntax features follow this path:

1. **Parser** — `tree-sitter-java` must support the new syntax. Check `tree-sitter-java` version in `Cargo.toml` and its grammar changelog.
2. **Indexer** (`src/index/codebase.rs` or `src/language/java/class_parser.rs`) — if the feature produces new class-file structures or source-level declarations, update the source or bytecode indexer.
3. **Location classification** (`src/language/java/location/`) — if the cursor can be positioned inside the new syntax, add a new `CursorLocation` variant or extend an existing handler.
4. **Expression typing** (`src/language/java/expression_typing.rs`) — if the new syntax produces a typed expression, add a case to `resolve_expression_type_ast`.
5. **Flow analysis** (`src/language/java/flow.rs`) — if the feature introduces narrowing (e.g., new pattern matching forms), extend `extract_instanceof_true_branch_overrides`.
6. **Providers** — update or add completion providers to surface new syntax in completion.
7. **Inlay hints** (`src/language/java/inlay_hints.rs`) — extend `collect_var_hints` or `collect_parameter_hints` if the feature introduces new `var`-like inference sites.
8. **Snapshot tests** — add snapshot tests covering the new behavior.

---

## Debugging Tips

### Enable tracing

```sh
RUST_LOG=java_analyzer=debug cargo run
# Or filter to a specific module:
RUST_LOG=java_analyzer::completion=trace cargo run
```

Logs are written to stderr; the LSP client typically surfaces them in its output panel.

### Tree-sitter playground

The [Tree-sitter playground](https://tree-sitter.github.io/tree-sitter/7-playground.html) lets you paste Java code and inspect the parse tree interactively. Use the [Treesitter inspector script](https://gist.github.com/cubewhy/7a43196d323488db4c4053f1c5126f9f) locally for offline inspection.

### Inspect the index

Add temporary `tracing::debug!` calls to `WorkspaceIndex` or `BucketIndex` methods to dump what classes are indexed for a given module or package.

### Snapshot a completion result

In a test, use `insta::assert_ron_snapshot!(candidates)` to capture the full provider output and diff it against future changes.

---

## Code Conventions

- **No `unwrap()`** in non-test code on fallible paths; use `?`, `if let`, or `match`.
- **Internal names** use JVM slash notation (`java/util/List`); source-facing names use dot notation (`java.util.List`). Never mix them silently.
- **`TypeName`** is the only semantic type currency crossing module boundaries. Do not pass raw descriptor strings into semantic logic.
- **`IndexView`** is always read-only and short-lived (per request). Never store it.
- **`DashMap`** entries must not be held across `await` points.
- **Blocking work** (I/O, subprocess) must be wrapped in `tokio::task::spawn_blocking`.
- Prefer `Arc<str>` over `String` for interned names stored in class metadata.
- Snapshot tests (`insta`) are the primary regression guard for semantic correctness — keep them up to date.

---

## Key Design Invariants

These are documented in detail in `docs/semantic-pipeline.md`. Summary:

1. **Representation boundary** — keep source text, `TypeName`, erased owner, and JVM descriptors in separate roles.
2. **Owner lookup** — always use `TypeName::erased_internal()` (the base name without generics) as the key into `IndexView`.
3. **Expected type** — use `typed_expr_ctx.expected_type` as the canonical expected-type output; do not invent parallel paths.
4. **Relaxed resolution** — on inner generic failure, preserve the base type and mark `Partial`; never drop to `None` when the base is known.
5. **Compatibility levels** — `Exact` only when clearly proven; `Partial` when plausibly compatible; `Incompatible` only when clearly wrong.
6. **Provider isolation** — providers must not mutate `SemanticContext`; all enrichment happens before provider dispatch.

---

## VS Code Extension Development

The extension lives in `editors/code/`. It is a standard VS Code Language Client extension.

```sh
cd editors/code
pnpm install       # install dependencies
pnpm run build     # bundle with esbuild into dist/extension.js
```

The extension:
- Launches the `java-analyzer` binary as a child process (stdio LSP).
- Passes workspace configuration (`jdkPath`, `decompilerPath`, `decompilerBackend`) to the server.
- Registers file watchers for Gradle build files.
- Supports Kotlin files in addition to Java for semantic token highlighting.

To test the extension locally, open the `editors/code/` folder in VS Code and press **F5** to launch an Extension Development Host.
