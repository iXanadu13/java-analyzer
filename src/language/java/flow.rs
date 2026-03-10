use std::collections::HashMap;
use std::sync::Arc;

use tree_sitter::Node;

use crate::language::java::JavaContextExtractor;
use crate::language::java::type_ctx::SourceTypeCtx;
use crate::semantic::LocalVar;
use crate::semantic::types::type_name::TypeName;

pub fn extract_instanceof_true_branch_overrides(
    ctx: &JavaContextExtractor,
    cursor_node: Option<Node>,
    type_ctx: &SourceTypeCtx,
    locals: &[LocalVar],
) -> HashMap<Arc<str>, TypeName> {
    let mut out: HashMap<Arc<str>, TypeName> = HashMap::new();
    let mut current = cursor_node;

    while let Some(node) = current {
        if node.kind() == "if_statement"
            && let Some(consequence) = node.child_by_field_name("consequence")
        {
            // Restrict narrowing to the true branch subtree only.
            if node_contains_offset(consequence, ctx.offset)
                && let Some(condition) = node.child_by_field_name("condition")
                && let Some((name, ty)) = parse_instanceof_narrowing(ctx, condition, type_ctx)
                && locals.iter().any(|lv| lv.name.as_ref() == name.as_ref())
            {
                // Walk from inner to outer `if`; keep inner-most fact on conflict.
                out.entry(name).or_insert(ty);
            }
        }
        current = node.parent();
    }

    out
}

fn node_contains_offset(node: Node, offset: usize) -> bool {
    node.start_byte() <= offset && offset <= node.end_byte()
}

fn parse_instanceof_narrowing(
    ctx: &JavaContextExtractor,
    condition: Node,
    type_ctx: &SourceTypeCtx,
) -> Option<(Arc<str>, TypeName)> {
    // if_statement.condition is parenthesized_expression; unwrap to payload expression.
    let expr = condition
        .child_by_field_name("expression")
        .or_else(|| first_named_child(condition))?;
    if expr.kind() != "instanceof_expression" {
        return None;
    }

    let left = expr.child_by_field_name("left")?;
    if left.kind() != "identifier" {
        return None;
    }
    let var_name = ctx.node_text(left).trim();
    if var_name.is_empty() {
        return None;
    }

    let right = expr.child_by_field_name("right")?;
    let right_text = ctx.node_text(right).trim();
    if right_text.is_empty() {
        return None;
    }
    let narrowed = type_ctx.resolve_type_name_relaxed(right_text)?.ty;

    Some((Arc::from(var_name), narrowed))
}

fn first_named_child(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).next()
}
