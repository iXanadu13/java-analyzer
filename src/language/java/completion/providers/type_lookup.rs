use crate::{
    completion::fuzzy,
    index::{ClassMetadata, IndexView},
    semantic::{context::SemanticContext, types::symbol_resolver::SymbolResolver},
};
use rust_asm::constants::{ACC_PRIVATE, ACC_PROTECTED, ACC_PUBLIC};
use std::sync::Arc;

pub(crate) fn qualified_nested_type_matches(
    prefix: &str,
    ctx: &SemanticContext,
    index: &IndexView,
) -> Vec<Arc<ClassMetadata>> {
    let Some(dot) = prefix.rfind('.') else {
        return vec![];
    };
    let qualifier = prefix[..dot].trim();
    let member_prefix = prefix[dot + 1..].trim();
    if qualifier.is_empty() {
        return vec![];
    }

    let resolver = SymbolResolver::new(index);
    let Some(owner_internal) = resolver.resolve_type_name(ctx, qualifier) else {
        return vec![];
    };

    visible_direct_inner_classes(&owner_internal, ctx, index)
        .into_iter()
        .filter(|inner| {
            member_prefix.is_empty()
                || fuzzy::fuzzy_match(
                    &member_prefix.to_lowercase(),
                    &inner.direct_name().to_lowercase(),
                )
                .is_some()
        })
        .collect()
}

pub(crate) fn visible_direct_inner_classes(
    owner_internal: &str,
    ctx: &SemanticContext,
    index: &IndexView,
) -> Vec<Arc<ClassMetadata>> {
    index
        .direct_inner_classes_of(owner_internal)
        .into_iter()
        .filter(|inner| is_direct_inner_class_accessible(inner, owner_internal, ctx))
        .collect()
}

fn is_direct_inner_class_accessible(
    inner: &ClassMetadata,
    owner_internal: &str,
    ctx: &SemanticContext,
) -> bool {
    let same_owner = ctx.enclosing_internal_name.as_deref() == Some(owner_internal);
    let same_package =
        inner.package.as_deref().unwrap_or("") == ctx.effective_package().unwrap_or("");

    match inner.access_flags {
        flags if flags & ACC_PUBLIC != 0 => true,
        flags if flags & ACC_PRIVATE != 0 => same_owner,
        flags if flags & ACC_PROTECTED != 0 => same_owner || same_package,
        _ => same_owner || same_package,
    }
}
