use super::candidate::CompletionCandidate;
use crate::index::{IndexScope, WorkspaceIndex};
use crate::semantic::SemanticContext;

pub trait CompletionProvider: Send + Sync {
    fn name(&self) -> &'static str;

    fn provide(
        &self,
        scope: IndexScope,
        ctx: &SemanticContext,
        index: &mut WorkspaceIndex,
    ) -> Vec<CompletionCandidate>;
}
