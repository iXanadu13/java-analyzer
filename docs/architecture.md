| `KeywordProvider` | Java keywords |
| `AnnotationProvider` | Annotation types (`@...`) |
| `SnippetProvider` | Code snippets (class body, method stubs) |
| `OverrideProvider` | `@Override` method stubs |
| `IntrinsicMemberProvider` | Built-in pseudo-members (e.g., `.class`, `.length`) |
| `StatementLabelProvider` | `break`/`continue` label names |
| `NameSuggestionProvider` | Variable name suggestions from type name |

### Scoring and Post-Processing

After all providers produce candidates:
1. **Fuzzy scoring** (`nucleo-matcher`): scores each candidate against the typed prefix.
2. **Post-processor** (`post_processor.rs`): deduplicates, merges provenance labels, applies final sort, enforces `final_result_limit`.

## Build Integration Layer

`BuildIntegrationService` manages the lifecycle of workspace model extraction:
- Watches Gradle build files (`build.gradle`, `settings.gradle`, `libs.versions.toml`, etc.) via LSP `workspace/didChangeWatchedFiles`.
- On change or initialization, schedules a debounced reload.
- Reload runs `GradleIntegration::import()`: injects a Gradle init script that exports JSON model describing modules, source roots, and dependency JARs.
- Two init-script strategies exist: `legacy` (Gradle 4–6) and `modern` (Gradle 7+), selected by `GradleVersion` detection.
- On success, `Workspace::apply_model` re-indexes all JARs and source roots.

## Decompiler Layer

`DecompilerCache` caches decompiled source files on disk under `~/.cache/java-analyzer/decompiled/`.
- Supported backends: **Vineflower** and **CFR** (selected via `decompilerBackend` config).
- The cache key is derived from the JAR path + class entry name.
- On a go-to-definition into bytecode, the cache is checked first; on a miss, the backend is invoked via `java -jar <decompiler.jar>`.
- The active backend and decompiler JAR path are configurable via `JavaAnalyzerConfig`.

## Concurrency Model

| Concern | Mechanism |
|---|---|
| LSP request handling | `tokio` async tasks (tower-lsp) |
| Index reads | `DashMap` (lock-free concurrent reads) |
| Index writes | `parking_lot::RwLock` per module |
| Document store | `parking_lot::RwLock` |
| JAR / codebase indexing | `rayon` parallel iterators |
| Config / build services | `tokio::sync::RwLock` |
| Workspace model | `parking_lot::RwLock` (sync fast path) |

Heavy blocking work (JDK indexing, Gradle import, decompilation) is dispatched with `tokio::task::spawn_blocking` to avoid stalling the async executor.
