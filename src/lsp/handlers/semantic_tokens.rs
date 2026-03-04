use std::sync::Arc;
use tower_lsp::lsp_types::*;

use crate::language::{LanguageRegistry, TokenCollector};
use crate::workspace::Workspace;

pub async fn handle_semantic_tokens(
    registry: Arc<LanguageRegistry>,
    workspace: Arc<Workspace>,
    params: SemanticTokensParams,
) -> Option<SemanticTokensResult> {
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

    let data = workspace.documents.with_doc(&uri, |doc| {
        let tree = doc.tree.as_ref()?;
        let mut collector = TokenCollector::new(doc.text.as_bytes(), &doc.rope, lang);
        collector.collect(tree.root_node());
        Some(collector.finish())
    })??;

    Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data,
    }))
}
