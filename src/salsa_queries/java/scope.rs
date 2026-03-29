use super::common::find_ancestor_of_kind;
use crate::salsa_db::SourceFile;
use crate::salsa_queries::Db;
use crate::salsa_queries::MethodSummaryData;
use std::sync::Arc;
use tree_sitter_utils::traversal::find_node_by_offset;

#[salsa::tracked]
pub fn find_java_enclosing_class_name(
    db: &dyn Db,
    file: SourceFile,
    offset: usize,
) -> Option<Arc<str>> {
    if let Some((name, _, _)) =
        crate::salsa_queries::semantic::find_enclosing_class_bounds(db, file, offset)
    {
        Some(name)
    } else {
        None
    }
}

#[salsa::tracked]
pub fn extract_java_enclosing_method(
    db: &dyn Db,
    file: SourceFile,
    offset: usize,
) -> Option<Arc<MethodSummaryData>> {
    let content: Arc<str> = Arc::from(file.content(db).as_str());
    let tree = crate::salsa_queries::parse::parse_tree(db, file)?;
    let root = tree.root_node();
    let extractor = crate::language::java::JavaContextExtractor::new_with_overview(
        Arc::clone(&content),
        offset.min(content.len()),
        None,
    );
    let cursor_node = extractor.find_cursor_node(root);
    let package = crate::salsa_queries::parse::extract_package(db, file);
    let imports = crate::salsa_queries::parse::extract_imports(db, file);
    let type_ctx = crate::language::java::type_ctx::SourceTypeCtx::from_view(
        package,
        imports.as_ref().clone(),
        crate::salsa_queries::get_index_view_for_context(
            db,
            crate::index::ModuleId::ROOT,
            crate::index::ClasspathId::Main,
            None,
        ),
    );

    cursor_node
        .and_then(|node| find_ancestor_of_kind(node, "method_declaration"))
        .or_else(|| find_node_by_offset(root, "method_declaration", offset))
        .or_else(|| crate::language::java::utils::find_enclosing_method_in_error(root, offset))
        .and_then(|node| {
            crate::language::java::members::parse_method_node(&extractor, &type_ctx, node)
        })
        .and_then(convert_current_member_to_method_data)
        .map(Arc::new)
}

#[salsa::tracked]
pub fn count_java_locals_in_scope(db: &dyn Db, file: SourceFile, offset: usize) -> usize {
    let Some((method_start, method_end)) =
        crate::salsa_queries::semantic::find_enclosing_method_bounds(db, file, offset)
    else {
        return 0;
    };

    let metadata = crate::salsa_queries::semantic::extract_method_locals_metadata(
        db,
        file,
        method_start,
        method_end,
    );
    metadata.local_count
}

fn convert_current_member_to_method_data(
    member: crate::semantic::context::CurrentClassMember,
) -> Option<MethodSummaryData> {
    match member {
        crate::semantic::context::CurrentClassMember::Method(method) => Some(MethodSummaryData {
            name: Arc::clone(&method.name),
            descriptor: method.desc(),
            param_names: method.params.param_names(),
            access_flags: method.access_flags,
            is_synthetic: method.is_synthetic,
            generic_signature: method.generic_signature.clone(),
            return_type: method.return_type.clone(),
        }),
        crate::semantic::context::CurrentClassMember::Field(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{ClassMetadata, ClassOrigin, WorkspaceIndex, WorkspaceIndexHandle};
    use crate::salsa_db::{Database, FileId, SourceFile};
    use indoc::indoc;
    use tower_lsp::lsp_types::Url;

    fn minimal_class(internal_name: &str) -> ClassMetadata {
        let (package, name) = internal_name
            .rsplit_once('/')
            .map(|(package, name)| (Some(Arc::from(package)), Arc::from(name)))
            .unwrap_or((None, Arc::from(internal_name)));
        ClassMetadata {
            package,
            name,
            internal_name: Arc::from(internal_name),
            super_name: None,
            interfaces: vec![],
            annotations: vec![],
            methods: vec![],
            fields: vec![],
            access_flags: 0,
            generic_signature: None,
            inner_class_of: None,
            origin: ClassOrigin::Unknown,
        }
    }

    #[test]
    fn extract_java_enclosing_method_resolves_param_types_from_index_view() {
        let source = indoc! {r#"
            class Test {
                void demo(StringBuilder builder) {
                    buil/*caret*/
                }
            }
        "#};
        let offset = source.find("/*caret*/").expect("caret marker");
        let source = source.replacen("/*caret*/", "", 1);
        let workspace_index = WorkspaceIndexHandle::new(WorkspaceIndex::new());
        workspace_index.update(|index| {
            index.add_jdk_classes(vec![minimal_class("java/lang/StringBuilder")]);
        });
        let db = Database::with_workspace_index(workspace_index);
        let uri = Url::parse("file:///test/Test.java").unwrap();
        let file = SourceFile::new(&db, FileId::new(uri), source, Arc::from("java"));

        let method = extract_java_enclosing_method(&db, file, offset).expect("enclosing method");

        assert_eq!(method.name.as_ref(), "demo");
        assert_eq!(method.descriptor.as_ref(), "(Ljava/lang/StringBuilder;)V");
        assert_eq!(method.param_names, vec![Arc::from("builder")]);
    }
}
