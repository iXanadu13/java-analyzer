use super::Db;
use crate::index::{ClassMetadata, NameTable};
use crate::salsa_db::SourceFile;
/// Java-specific Salsa queries
///
/// These queries handle Java-specific parsing and analysis.
use std::sync::Arc;

/// Parse Java source and extract class metadata with full incremental support
///
/// This is the main entry point for Java file indexing.
/// Note: We return the classes directly, not wrapped in Arc, because Salsa
/// will handle the memoization.
pub fn parse_java_classes(db: &dyn Db, file: SourceFile) -> Vec<ClassMetadata> {
    let content = file.content(db);
    let file_id = file.file_id(db);

    // Get name table if available
    let name_table = get_name_table_for_java_file(db, file);

    let origin = crate::index::ClassOrigin::SourceFile(Arc::from(file_id.as_str()));

    crate::language::java::class_parser::parse_java_source(content, origin, name_table)
}

/// Get name table for a Java file's context
fn get_name_table_for_java_file(_db: &dyn Db, _file: SourceFile) -> Option<Arc<NameTable>> {
    // TODO: Query the workspace model to determine the file's context
    // For now, return None and let the parser work without name table
    None
}

/// Extract Java package declaration
pub fn extract_java_package(db: &dyn Db, file: SourceFile) -> Option<Arc<str>> {
    let content = file.content(db);
    crate::language::java::class_parser::extract_package_from_source(content)
}

/// Extract Java imports
pub fn extract_java_imports(db: &dyn Db, file: SourceFile) -> Vec<Arc<str>> {
    let content = file.content(db);
    crate::language::java::class_parser::extract_imports_from_source(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::salsa_db::{Database, FileId};
    use tower_lsp::lsp_types::Url;

    #[test]
    fn test_parse_java_classes() {
        let db = Database::default();
        let uri = Url::parse("file:///test/Test.java").unwrap();
        let file = SourceFile::new(
            &db,
            FileId::new(uri),
            "package com.example;\npublic class Test { void foo() {} }".to_string(),
            Arc::from("java"),
        );

        let classes = parse_java_classes(&db, file);
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].name.as_ref(), "Test");
        assert_eq!(classes[0].package.as_deref(), Some("com/example"));
        assert_eq!(classes[0].methods.len(), 1);
        assert_eq!(classes[0].methods[0].name.as_ref(), "foo");
    }

    #[test]
    fn test_extract_java_package() {
        let db = Database::default();
        let uri = Url::parse("file:///test/Test.java").unwrap();
        let file = SourceFile::new(
            &db,
            FileId::new(uri),
            "package org.example.test;\npublic class Test {}".to_string(),
            Arc::from("java"),
        );

        let package = extract_java_package(&db, file);
        assert_eq!(package.as_deref(), Some("org/example/test"));
    }

    #[test]
    fn test_extract_java_imports() {
        let db = Database::default();
        let uri = Url::parse("file:///test/Test.java").unwrap();
        let file = SourceFile::new(
            &db,
            FileId::new(uri),
            "import java.util.*;\nimport java.io.File;\npublic class Test {}".to_string(),
            Arc::from("java"),
        );

        let imports = extract_java_imports(&db, file);
        assert_eq!(imports.len(), 2);
    }
}
