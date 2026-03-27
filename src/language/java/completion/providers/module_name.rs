use std::sync::Arc;

use crate::{
    completion::{
        CandidateKind, CompletionCandidate,
        candidate::ReplacementMode,
        fuzzy,
        provider::{CompletionProvider, ProviderCompletionResult, ProviderSearchSpace},
    },
    index::{IndexScope, IndexView},
    semantic::context::{JavaModuleContextKind, SemanticContext},
};

pub struct ModuleNameProvider;

impl CompletionProvider for ModuleNameProvider {
    fn name(&self) -> &'static str {
        "module_name"
    }

    fn search_space(&self, _ctx: &SemanticContext) -> ProviderSearchSpace {
        ProviderSearchSpace::Broad
    }

    fn is_applicable(&self, ctx: &SemanticContext) -> bool {
        matches!(
            ctx.java_module_context,
            Some(JavaModuleContextKind::RequiresModule | JavaModuleContextKind::TargetModule)
        )
    }

    fn provide(
        &self,
        _scope: IndexScope,
        ctx: &SemanticContext,
        _index: &IndexView,
        _request: Option<&crate::lsp::request_context::RequestContext>,
        _limit: Option<usize>,
    ) -> crate::lsp::request_cancellation::RequestResult<ProviderCompletionResult> {
        let prefix = ctx.query.as_str();
        let prefix_lower = prefix.to_lowercase();
        let mut results = ctx
            .java_module_names
            .iter()
            .filter(|module_name| {
                ctx.current_java_module_name
                    .as_deref()
                    .is_none_or(|current| current != module_name.as_ref())
            })
            .filter_map(|module_name| {
                let score = if prefix.is_empty() {
                    Some(50.0)
                } else if module_name.to_lowercase().starts_with(&prefix_lower) {
                    Some(90.0)
                } else {
                    fuzzy::fuzzy_match(&prefix_lower, &module_name.to_lowercase())
                        .map(|score| 60.0 + score as f32 * 0.1)
                }?;
                Some(
                    CompletionCandidate::new(
                        Arc::clone(module_name),
                        module_name.to_string(),
                        CandidateKind::Package,
                        self.name(),
                    )
                    .with_detail(format!("module {}", module_name))
                    .with_replacement_mode(ReplacementMode::PackagePath)
                    .with_filter_text(module_name.to_string())
                    .with_score(score),
                )
            })
            .collect::<Vec<_>>();
        results.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.label.as_ref().cmp(right.label.as_ref()))
        });
        Ok(results.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic::context::CursorLocation;

    #[test]
    fn filters_current_module_from_requires_completion() {
        let ctx = crate::semantic::SemanticContext::new(
            CursorLocation::Expression {
                prefix: "com.example".to_string(),
            },
            "com.example",
            vec![],
            None,
            None,
            None,
            vec![],
        )
        .with_java_module_context(Some(JavaModuleContextKind::RequiresModule))
        .with_current_java_module_name(Some(Arc::from("com.example.app")))
        .with_java_module_names(vec![
            Arc::from("com.example.app"),
            Arc::from("com.example.shared"),
        ]);

        let results = ModuleNameProvider
            .provide_test(
                IndexScope {
                    module: crate::index::ModuleId::ROOT,
                },
                &ctx,
                &crate::index::WorkspaceIndex::new().view(IndexScope {
                    module: crate::index::ModuleId::ROOT,
                }),
                None,
            )
            .candidates;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].label.as_ref(), "com.example.shared");
    }
}
