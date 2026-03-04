use std::sync::Arc;
use tower_lsp::lsp_types::*;

use crate::language::LanguageRegistry;
use crate::workspace::Workspace;

pub async fn handle_document_symbol(
    registry: Arc<LanguageRegistry>,
    workspace: Arc<Workspace>,
    params: DocumentSymbolParams,
) -> Option<DocumentSymbolResponse> {
    let uri = params.text_document.uri;

    let lang_id = workspace
        .documents
        .with_doc(&uri, |doc| doc.language_id.clone())?;

    let lang = registry.find(&lang_id)?;

    workspace.documents.with_doc_mut(&uri, |doc| {
        if doc.tree.is_none() {
            doc.tree = lang.parse_tree(&doc.text, None);
        }
    })?;

    let symbols = workspace.documents.with_doc(&uri, |doc| {
        let tree = doc.tree.as_ref()?;
        let root = tree.root_node();
        let bytes = doc.text.as_bytes();
        lang.collect_symbols(root, bytes)
    })??;

    Some(DocumentSymbolResponse::Nested(symbols))
}
