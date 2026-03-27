use crate::{
    completion::{
        CandidateKind, CompletionCandidate,
        provider::{CompletionProvider, ProviderCompletionResult},
    },
    index::{IndexScope, IndexView},
    language::java::module_info::{MODULE_DIRECTIVE_KEYWORDS, REQUIRES_MODIFIERS},
    semantic::context::{CursorLocation, SemanticContext},
};
use std::sync::Arc;

#[rustfmt::skip]
const JAVA_KEYWORDS: &[&str] = &[
    "abstract", "assert", "boolean", "break", "byte",
    "case", "catch", "char", "class", "const", "continue",
    "default", "do", "double", "else", "enum", "extends",
    "final", "finally", "float", "for", "goto", "if",
    "implements", "import", "instanceof", "int", "interface",
    "long", "native", "new", "package", "private", "protected",
    "public", "return", "short", "static", "strictfp", "super",
    "switch", "synchronized", "this", "throw", "throws", "transient",
    "try", "var", "void", "volatile", "while",
    // Java 17+
    "record", "sealed", "permits", "yield", "text",
];

pub struct KeywordProvider;

impl CompletionProvider for KeywordProvider {
    fn name(&self) -> &'static str {
        "keyword"
    }

    fn provide(
        &self,
        _scope: IndexScope,
        ctx: &SemanticContext,
        _index: &IndexView,
        _request: Option<&crate::lsp::request_context::RequestContext>,
        _limit: Option<usize>,
    ) -> crate::lsp::request_cancellation::RequestResult<ProviderCompletionResult> {
        let (prefix, keywords): (&str, &[&str]) = match (&ctx.location, ctx.java_module_context) {
            (
                CursorLocation::Expression { prefix },
                Some(crate::semantic::context::JavaModuleContextKind::DirectiveKeyword),
            ) => (prefix.as_str(), MODULE_DIRECTIVE_KEYWORDS),
            (
                CursorLocation::Expression { prefix },
                Some(crate::semantic::context::JavaModuleContextKind::RequiresModifier),
            ) => (prefix.as_str(), REQUIRES_MODIFIERS),
            (CursorLocation::Expression { prefix }, None) => (prefix.as_str(), JAVA_KEYWORDS),
            _ => return Ok(ProviderCompletionResult::default()),
        };

        let prefix_lower = prefix.to_lowercase();

        Ok(keywords
            .iter()
            .filter(|&&kw| kw.starts_with(&prefix_lower))
            .map(|&kw| {
                CompletionCandidate::new(
                    Arc::from(kw),
                    kw.to_string(),
                    CandidateKind::Keyword,
                    self.name(),
                )
            })
            .collect::<Vec<_>>()
            .into())
    }
}
