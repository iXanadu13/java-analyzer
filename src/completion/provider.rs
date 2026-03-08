use super::candidate::CompletionCandidate;
use crate::index::{IndexScope, IndexView};
use crate::semantic::SemanticContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSearchSpace {
    Narrow,
    Broad,
}

#[derive(Debug, Default)]
pub struct ProviderCompletionResult {
    pub candidates: Vec<CompletionCandidate>,
    pub is_incomplete: bool,
}

pub trait CompletionProvider: Send + Sync {
    fn name(&self) -> &'static str;

    fn is_applicable(&self, _ctx: &SemanticContext) -> bool {
        true
    }

    fn search_space(&self, _ctx: &SemanticContext) -> ProviderSearchSpace {
        ProviderSearchSpace::Narrow
    }

    fn provide(
        &self,
        scope: IndexScope,
        ctx: &SemanticContext,
        index: &IndexView,
    ) -> Vec<CompletionCandidate>;

    fn provide_with_limit(
        &self,
        scope: IndexScope,
        ctx: &SemanticContext,
        index: &IndexView,
        _limit: Option<usize>,
    ) -> ProviderCompletionResult {
        ProviderCompletionResult {
            candidates: self.provide(scope, ctx, index),
            is_incomplete: false,
        }
    }
}
