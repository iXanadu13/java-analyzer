use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use tower_lsp::lsp_types::{SemanticToken, Url};
use tree_sitter::Tree;

use crate::build_integration::SourceRootId;
use crate::index::{ClasspathId, ModuleId};
use crate::semantic::SemanticContext;

use super::source_file::SourceFile;

const SEMANTIC_CONTEXT_CACHE_LIMIT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SemanticContextCacheKey {
    pub document_version: i32,
    pub workspace_version: u64,
    pub module: ModuleId,
    pub classpath: ClasspathId,
    pub source_root: Option<SourceRootId>,
    pub offset: usize,
    pub trigger: Option<char>,
}

/// Per-document mutable LSP state.
///
/// `Document` owns the current [`SourceFile`] snapshot plus any LSP-level
/// caches that survive across re-parses (e.g. the semantic-token result
/// cache used for incremental delta responses).
///
/// Immutable analysis (completion, inlay hints, semantic tokens, …) should
/// read through `doc.source()` and never access the fields of `Document`
/// directly except to update LSP caches.
#[derive(Debug)]
pub struct Document {
    /// The current parsed snapshot.  Replaced atomically on every
    /// `didChange` / `didOpen` / `didSave`.
    source: Arc<SourceFile>,

    /// Cached semantic token result: `(result_id, flat token data)`.
    /// Keyed on `result_id` (= document version as string) and invalidated
    /// whenever `source` is replaced with a new version.
    pub semantic_token_cache: Option<(String, Vec<SemanticToken>)>,

    /// Cached semantic contexts for the current document version.
    semantic_context_cache: HashMap<SemanticContextCacheKey, Arc<SemanticContext>>,
}

impl Document {
    /// Create a new `Document` from an initial [`SourceFile`].
    pub fn new(source: SourceFile) -> Self {
        Self {
            source: Arc::new(source),
            semantic_token_cache: None,
            semantic_context_cache: HashMap::new(),
        }
    }

    /// Read access to the current source snapshot.
    #[inline]
    pub fn source(&self) -> &Arc<SourceFile> {
        &self.source
    }

    #[inline]
    pub fn snapshot(&self) -> Arc<SourceFile> {
        Arc::clone(&self.source)
    }

    /// Replace the current source snapshot with a new one.
    /// Invalidates all version-sensitive caches.
    pub fn update_source(&mut self, source: SourceFile) {
        self.source = Arc::new(source);
        self.semantic_token_cache = None;
        self.semantic_context_cache.clear();
    }

    /// Attach an already-incremented tree to the current source, producing a
    /// new `SourceFile` with the updated tree.  Used by the `did_change`
    /// handler after `tree.edit` + `parser.parse(…, Some(&old))`.
    pub fn set_tree(&mut self, tree: Option<Tree>) {
        // Avoid a full clone of the Arc<SourceFile> by replacing source
        // with a new SourceFile that shares everything except the tree.
        let prev = Arc::unwrap_or_clone(Arc::clone(&self.source));
        self.source = Arc::new(prev.with_tree(tree));
        self.semantic_context_cache.clear();
        // Tree change does not invalidate semantic-token cache by itself;
        // text already changed before the tree was updated.
    }

    pub fn cached_semantic_context(
        &self,
        key: &SemanticContextCacheKey,
    ) -> Option<Arc<SemanticContext>> {
        self.semantic_context_cache.get(key).cloned()
    }

    pub fn cache_semantic_context(
        &mut self,
        key: SemanticContextCacheKey,
        context: Arc<SemanticContext>,
    ) {
        if self.semantic_context_cache.len() >= SEMANTIC_CONTEXT_CACHE_LIMIT {
            self.semantic_context_cache.clear();
        }
        self.semantic_context_cache.insert(key, context);
    }

    // ── Convenience pass-throughs ────────────────────────────────────────

    pub fn uri(&self) -> &Url {
        &self.source.uri
    }

    pub fn language_id(&self) -> &str {
        &self.source.language_id
    }

    pub fn version(&self) -> i32 {
        self.source.version
    }

    pub fn text(&self) -> &str {
        self.source.text()
    }
}

/// Thread-safe store of open LSP documents.
pub struct DocumentStore {
    docs: DashMap<Url, Document>,
}

impl DocumentStore {
    pub fn new() -> Self {
        Self {
            docs: DashMap::new(),
        }
    }

    pub fn open(&self, doc: Document) {
        self.docs.insert(doc.uri().clone(), doc);
    }

    pub fn close(&self, uri: &Url) {
        self.docs.remove(uri);
    }

    /// Read-only access — do NOT `.await` inside `f`.
    pub fn with_doc<R>(&self, uri: &Url, f: impl FnOnce(&Document) -> R) -> Option<R> {
        self.docs.get(uri).map(|d| f(&d))
    }

    /// Mutable access — do NOT `.await` inside `f`.
    pub fn with_doc_mut<R>(&self, uri: &Url, f: impl FnOnce(&mut Document) -> R) -> Option<R> {
        self.docs.get_mut(uri).map(|mut d| f(&mut d))
    }

    pub fn snapshot_documents(&self) -> Vec<(Url, String, String)> {
        self.docs
            .iter()
            .map(|entry| {
                let doc = entry.value();
                (
                    doc.uri().clone(),
                    doc.language_id().to_owned(),
                    doc.text().to_owned(),
                )
            })
            .collect()
    }
}

impl Default for DocumentStore {
    fn default() -> Self {
        Self::new()
    }
}
