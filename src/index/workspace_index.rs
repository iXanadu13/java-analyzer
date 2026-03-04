use std::sync::Arc;

use crate::index::{ClassMetadata, ClassOrigin, GlobalIndex, NameTable};
use crate::index::scope::IndexScope;
use crate::index::{FieldSummary, MethodSummary};

pub struct WorkspaceIndex {
    inner: GlobalIndex,
    // TODO: per-module source index
    // TODO: per-module jar/classpath index
    // TODO: shared JDK index
    // TODO: dependency graph and lookup order
}

impl WorkspaceIndex {
    pub fn new() -> Self {
        Self {
            inner: GlobalIndex::new(),
        }
    }

    pub fn add_jdk_classes(&mut self, classes: Vec<ClassMetadata>) {
        self.inner.add_classes(classes);
    }

    pub fn add_jar_classes(&mut self, _scope: IndexScope, classes: Vec<ClassMetadata>) {
        self.inner.add_classes(classes);
    }

    pub fn update_source(
        &mut self,
        _scope: IndexScope,
        origin: ClassOrigin,
        classes: Vec<ClassMetadata>,
    ) {
        self.inner.update_source(origin, classes);
    }

    pub fn get_class(
        &self,
        _scope: IndexScope,
        internal_name: &str,
    ) -> Option<Arc<ClassMetadata>> {
        self.inner.get_class(internal_name)
    }

    pub fn get_source_type_name(
        &self,
        _scope: IndexScope,
        internal: &str,
    ) -> Option<String> {
        self.inner.get_source_type_name(internal)
    }

    pub fn get_classes_by_simple_name(
        &self,
        _scope: IndexScope,
        simple_name: &str,
    ) -> &[Arc<ClassMetadata>] {
        self.inner.get_classes_by_simple_name(simple_name)
    }

    pub fn classes_in_package(
        &self,
        _scope: IndexScope,
        pkg: &str,
    ) -> &[Arc<ClassMetadata>] {
        self.inner.classes_in_package(pkg)
    }

    pub fn has_package(&self, _scope: IndexScope, pkg: &str) -> bool {
        self.inner.has_package(pkg)
    }

    pub fn has_classes_in_package(&self, _scope: IndexScope, pkg: &str) -> bool {
        self.inner.has_classes_in_package(pkg)
    }

    pub fn resolve_imports(
        &self,
        _scope: IndexScope,
        imports: &[Arc<str>],
    ) -> Vec<Arc<ClassMetadata>> {
        self.inner.resolve_imports(imports)
    }

    pub fn collect_inherited_members(
        &self,
        _scope: IndexScope,
        class_internal: &str,
    ) -> (Vec<Arc<MethodSummary>>, Vec<Arc<FieldSummary>>) {
        self.inner.collect_inherited_members(class_internal)
    }

    pub fn mro(&self, _scope: IndexScope, class_internal: &str) -> Vec<Arc<ClassMetadata>> {
        self.inner.mro(class_internal)
    }

    pub fn fuzzy_autocomplete(
        &mut self,
        _scope: IndexScope,
        query: &str,
        limit: usize,
    ) -> Vec<Arc<str>> {
        self.inner.fuzzy_autocomplete(query, limit)
    }

    pub fn fuzzy_search_classes(
        &mut self,
        _scope: IndexScope,
        query: &str,
        limit: usize,
    ) -> Vec<Arc<ClassMetadata>> {
        self.inner.fuzzy_search_classes(query, limit)
    }

    pub fn exact_match_keys(&self, _scope: IndexScope) -> impl Iterator<Item = &Arc<str>> {
        self.inner.exact_match_keys()
    }

    pub fn class_count(&self, _scope: IndexScope) -> usize {
        self.inner.class_count()
    }

    pub fn iter_all_classes(&self, _scope: IndexScope) -> impl Iterator<Item = &Arc<ClassMetadata>> {
        self.inner.iter_all_classes()
    }

    pub fn build_name_table(&self, _scope: IndexScope) -> Arc<NameTable> {
        self.inner.build_name_table()
    }

    pub fn add_classes(&mut self, classes: Vec<ClassMetadata>) {
        self.inner.add_classes(classes);
    }
}

impl Default for WorkspaceIndex {
    fn default() -> Self {
        Self::new()
    }
}
