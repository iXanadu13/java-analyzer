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
    let doc = workspace.documents.get(&uri)?;

    let lang = registry.find(&doc.language_id)?;
    let mut parser = lang.make_parser();

    let tree = parser.parse(doc.content.as_ref(), None)?;

    let rope = ropey::Rope::from_str(doc.content.as_ref());

    let mut collector = TokenCollector::new(doc.content.as_bytes(), &rope, lang);
    collector.collect(tree.root_node());

    Some(SemanticTokensResult::Tokens(SemanticTokens {
        result_id: None,
        data: collector.finish(),
    }))
}
