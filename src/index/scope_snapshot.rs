use std::sync::{Arc, OnceLock};

use rustc_hash::FxHashSet;
use smallvec::SmallVec;

use crate::build_integration::SourceRootId;
use crate::index::{BucketIndex, ClasspathId, ModuleId, NameTable};

pub type AnalysisContextKey = (ModuleId, ClasspathId, Option<SourceRootId>);

/// Immutable scope topology for one analysis context.
///
/// This is intentionally compact: it captures only visibility order and
/// reusable declaration summaries. Expensive semantic joins live in
/// request-scoped query layers like `IndexView`.
pub struct ScopeSnapshot {
    key: AnalysisContextKey,
    layers: SmallVec<Arc<BucketIndex>, 8>,
    jar_paths: Vec<Arc<str>>,
    name_table: OnceLock<Arc<NameTable>>,
}

impl ScopeSnapshot {
    pub fn new(
        module_id: ModuleId,
        classpath: ClasspathId,
        source_root: Option<SourceRootId>,
        layers: SmallVec<Arc<BucketIndex>, 8>,
        jar_paths: Vec<Arc<str>>,
    ) -> Self {
        Self {
            key: (module_id, classpath, source_root),
            layers,
            jar_paths,
            name_table: OnceLock::new(),
        }
    }

    pub fn from_layers(layers: SmallVec<Arc<BucketIndex>, 8>) -> Self {
        Self::new(ModuleId::ROOT, ClasspathId::Main, None, layers, Vec::new())
    }

    pub fn key(&self) -> AnalysisContextKey {
        self.key
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn layers(&self) -> &[Arc<BucketIndex>] {
        &self.layers
    }

    pub fn jar_paths(&self) -> &[Arc<str>] {
        &self.jar_paths
    }

    pub fn jar_count(&self) -> usize {
        self.jar_paths.len()
    }

    pub fn build_name_table(&self) -> Arc<NameTable> {
        Arc::clone(self.name_table.get_or_init(|| {
            let mut names = Vec::new();
            let mut seen: FxHashSet<Arc<str>> = Default::default();
            for layer in &self.layers {
                let layer_table = layer.build_name_table();
                for name in layer_table.iter() {
                    if seen.insert(Arc::clone(name)) {
                        names.push(Arc::clone(name));
                    }
                }
            }
            tracing::debug!(
                module = self.key.0.0,
                classpath = ?self.key.1,
                source_root = ?self.key.2.map(|id| id.0),
                layer_count = self.layers.len(),
                name_count = names.len(),
                phase = "scope_snapshot",
                "build NameTable from scope snapshot"
            );
            NameTable::from_names(names)
        }))
    }
}
