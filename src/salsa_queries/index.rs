//! Index queries - handle class extraction and name table construction
//!
//! These queries build on parse queries and provide the indexed view of code.
use super::Db;
use crate::build_integration::SourceRootId;
use crate::index::{ClassMetadata, ClasspathId, ModuleId, NameTable};
use crate::salsa_db::SourceFile;

use std::sync::Arc;

/// A wrapper for class extraction results that can be used with Salsa
///
/// We store a hash of the classes for change detection, and the actual
/// classes are stored separately (not in Salsa).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassExtractionResult {
    /// Hash of the extracted classes for change detection
    pub content_hash: u64,
    /// Number of classes extracted
    pub class_count: usize,
}

/// Extract class metadata from a source file
///
/// This is the main indexing query - it parses the file and extracts all
/// class definitions, methods, fields, etc.
///
/// Note: This returns a lightweight result for Salsa tracking. The actual
/// classes are retrieved separately via `get_extracted_classes`.
#[salsa::tracked]
pub fn extract_classes(db: &dyn Db, file: SourceFile) -> ClassExtractionResult {
    let lang_id = file.language_id(db);

    let classes = if lang_id.as_ref() == "java" {
        crate::salsa_queries::java::parse_java_classes(db, file)
    } else if lang_id.as_ref() == "kotlin" {
        crate::salsa_queries::kotlin::parse_kotlin_classes(db, file)
    } else {
        vec![]
    };

    // Compute hash for change detection
    let content_hash = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        classes.len().hash(&mut hasher);
        for class in &classes {
            class.internal_name.hash(&mut hasher);
            class.methods.len().hash(&mut hasher);
            class.fields.len().hash(&mut hasher);
        }
        hasher.finish()
    };

    ClassExtractionResult {
        content_hash,
        class_count: classes.len(),
    }
}

/// Get the actual extracted classes for a file
///
/// This is not a Salsa query - it directly parses the file.
/// Call this after `extract_classes` to get the actual class data.
pub fn get_extracted_classes(db: &dyn Db, file: SourceFile) -> Vec<ClassMetadata> {
    let lang_id = file.language_id(db);

    if lang_id.as_ref() == "java" {
        crate::salsa_queries::java::parse_java_classes(db, file)
    } else if lang_id.as_ref() == "kotlin" {
        crate::salsa_queries::kotlin::parse_kotlin_classes(db, file)
    } else {
        vec![]
    }
}

/// Build a name table for a specific analysis context
///
/// This queries the workspace index directly (not memoized by Salsa).
pub fn build_name_table_for_context(
    db: &dyn Db,
    module_id: ModuleId,
    classpath: ClasspathId,
    source_root: Option<SourceRootId>,
) -> Arc<NameTable> {
    let workspace_index = db.workspace_index();
    let index = workspace_index.read();

    index.build_name_table_for_analysis_context(module_id, classpath, source_root)
}

/// Get visible classpath JARs for a module and classpath
///
/// This queries the workspace index directly (not memoized by Salsa).
pub fn visible_classpath_for_context(
    db: &dyn Db,
    module_id: ModuleId,
    classpath: ClasspathId,
) -> Arc<Vec<Arc<str>>> {
    let workspace_index = db.workspace_index();
    let index = workspace_index.read();

    Arc::new(index.module_classpath_jars(module_id, classpath))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::salsa_db::{Database, FileId};
    use tower_lsp::lsp_types::Url;

    #[test]
    fn test_extract_classes() {
        let db = Database::default();
        let uri = Url::parse("file:///test/Test.java").unwrap();
        let file = SourceFile::new(
            &db,
            FileId::new(uri),
            "package com.example;\npublic class Test {}".to_string(),
            Arc::from("java"),
        );

        let result = extract_classes(&db, file);
        assert_eq!(result.class_count, 1);

        let classes = get_extracted_classes(&db, file);
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].name.as_ref(), "Test");
        assert_eq!(classes[0].package.as_deref(), Some("com/example"));
    }

    #[test]
    fn test_extract_classes_memoization() {
        let db = Database::default();
        let uri = Url::parse("file:///test/Test.java").unwrap();
        let file = SourceFile::new(
            &db,
            FileId::new(uri),
            "public class Test {}".to_string(),
            Arc::from("java"),
        );

        // First extraction
        let result1 = extract_classes(&db, file);

        // Second extraction - should return same result (memoized)
        let result2 = extract_classes(&db, file);

        assert_eq!(result1, result2);
    }

    #[test]
    fn test_extract_classes_invalidation() {
        use salsa::Setter;

        let mut db = Database::default();
        let uri = Url::parse("file:///test/Test.java").unwrap();
        let file = SourceFile::new(
            &db,
            FileId::new(uri),
            "public class Test {}".to_string(),
            Arc::from("java"),
        );

        let result1 = extract_classes(&db, file);
        assert_eq!(result1.class_count, 1);

        // Modify to add another class
        file.set_content(&mut db)
            .to("public class Test {}\nclass Another {}".to_string());

        let result2 = extract_classes(&db, file);
        assert_eq!(result2.class_count, 2);
        assert_ne!(result1.content_hash, result2.content_hash);
    }
}
