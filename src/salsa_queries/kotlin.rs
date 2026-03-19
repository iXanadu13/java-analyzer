use super::Db;
use crate::index::ClassMetadata;
use crate::salsa_db::SourceFile;
/// Kotlin-specific Salsa queries (placeholder)
///
/// These queries will handle Kotlin-specific parsing and analysis.
/// Currently a placeholder - to be implemented after Java is stable.
use std::sync::Arc;

/// Parse Kotlin source and extract class metadata
///
/// TODO: Implement full Kotlin parsing with incremental support
pub fn parse_kotlin_classes(_db: &dyn Db, _file: SourceFile) -> Vec<ClassMetadata> {
    // Placeholder - return empty for now
    vec![]
}

/// Extract Kotlin package declaration
///
/// TODO: Implement Kotlin package extraction
pub fn extract_kotlin_package(_db: &dyn Db, _file: SourceFile) -> Option<Arc<str>> {
    None
}

/// Extract Kotlin imports
///
/// TODO: Implement Kotlin import extraction
pub fn extract_kotlin_imports(_db: &dyn Db, _file: SourceFile) -> Vec<Arc<str>> {
    vec![]
}
