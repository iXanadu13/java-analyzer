use crate::language::java::JavaContextExtractor;
use crate::language::java::utils::strip_sentinel;
use crate::semantic::CursorLocation;
use tree_sitter::Node;

use super::handlers;
use super::text::{detect_new_keyword_before_cursor, detect_trailing_dot_in_text};
use super::utils::{cursor_truncated_text, find_preceding_named_sibling};

/// cursor_node is ERROR (cases 1, 2, 3, 5, 6, 8, 12)
pub(super) fn handle_error(
    ctx: &JavaContextExtractor,
    error_node: Node,
    cursor_node: Node,
    trigger_char: Option<char>,
) -> (CursorLocation, String) {
    let parent_kind = error_node.parent().map(|p| p.kind()).unwrap_or("");

    // ERROR inside argument_list
    if parent_kind == "argument_list" {
        let p = error_node.parent().unwrap();
        let is_trailing_dot = {
            let mut wc = error_node.walk();
            let ch: Vec<_> = error_node.children(&mut wc).collect();
            ch.len() == 1 && ch[0].kind() == "."
        };
        if is_trailing_dot && let Some(recv) = find_preceding_named_sibling(error_node, p) {
            let receiver_expr = ctx.node_text(recv).to_string();
            return (
                CursorLocation::MemberAccess {
                    receiver_semantic_type: None,
                    receiver_type: None,
                    member_prefix: String::new(),
                    receiver_expr,
                    arguments: None,
                },
                String::new(),
            );
        }
        return handlers::handle_argument_list(ctx, p);
    }

    // ERROR inside class_body (case 5: `prote`)
    if parent_kind == "class_body" {
        return handle_error_in_class_body(ctx, error_node, cursor_node);
    }

    // Source text analysis for ERROR in block or at program level
    let before = &ctx.source[..ctx.offset.min(ctx.source.len())];

    if is_import_context(before) {
        return handle_import_from_text(ctx, before);
    }

    // Priority 1: trailing dot → MemberAccess
    if let Some((receiver, member_prefix)) = detect_trailing_dot_in_text(before) {
        return (
            CursorLocation::MemberAccess {
                receiver_semantic_type: None,
                receiver_type: None,
                member_prefix: member_prefix.clone(),
                receiver_expr: receiver,
                arguments: None,
            },
            member_prefix,
        );
    }

    // Priority 2: `new ClassName` → ConstructorCall
    if let Some((class_prefix, expected_type)) = detect_new_keyword_before_cursor(before) {
        return (
            CursorLocation::ConstructorCall {
                class_prefix: class_prefix.clone(),
                expected_type,
            },
            class_prefix,
        );
    }

    // Priority 3: expression child followed by dot (cases 6, 8)
    if let Some(r) = detect_dot_after_expression_child(ctx, error_node) {
        return r;
    }

    // Priority 4: cursor is on identifier → Expression
    if matches!(cursor_node.kind(), "identifier" | "type_identifier") {
        return handlers::handle_identifier(ctx, cursor_node, trigger_char);
    }

    (
        CursorLocation::Expression {
            prefix: String::new(),
        },
        String::new(),
    )
}

pub(super) fn is_import_context(before_cursor: &str) -> bool {
    let last_stmt = before_cursor
        .rsplit([';', '{', '}'])
        .next()
        .unwrap_or(before_cursor)
        .trim_start();
    last_stmt.starts_with("import")
}

pub(super) fn handle_import_from_text(
    _ctx: &JavaContextExtractor,
    before: &str,
) -> (CursorLocation, String) {
    let last_stmt = before
        .rsplit([';', '{', '}'])
        .next()
        .unwrap_or(before)
        .trim_start();

    let after_import = last_stmt.strip_prefix("import").unwrap_or("").trim_start();
    let is_static = after_import.starts_with("static ");
    let prefix_raw = if is_static {
        after_import
            .strip_prefix("static")
            .unwrap_or("")
            .trim_start()
    } else {
        after_import
    };
    let prefix = strip_sentinel(prefix_raw);
    let query = prefix.rsplit('.').next().unwrap_or("").to_string();
    let location = if is_static {
        CursorLocation::ImportStatic { prefix }
    } else {
        CursorLocation::Import { prefix }
    };
    (location, query)
}

/// Case 5: ERROR in class_body
pub(super) fn handle_error_in_class_body(
    ctx: &JavaContextExtractor,
    error_node: Node,
    cursor_node: Node,
) -> (CursorLocation, String) {
    let ident = if matches!(cursor_node.kind(), "identifier" | "type_identifier") {
        Some(cursor_node)
    } else {
        let mut wc = error_node.walk();
        error_node
            .named_children(&mut wc)
            .find(|n| matches!(n.kind(), "identifier" | "type_identifier"))
    };

    if let Some(id) = ident {
        let text = cursor_truncated_text(ctx, id);
        let clean = strip_sentinel(&text);
        return (
            CursorLocation::Expression {
                prefix: clean.clone(),
            },
            clean,
        );
    }

    (
        CursorLocation::Expression {
            prefix: String::new(),
        },
        String::new(),
    )
}

/// Cases 6, 8: ERROR has expression child followed by dot in source text
fn detect_dot_after_expression_child(
    ctx: &JavaContextExtractor,
    error_node: Node,
) -> Option<(CursorLocation, String)> {
    let mut wc = error_node.walk();
    let children: Vec<Node> = error_node.children(&mut wc).collect();

    let expr_child = children.iter().rev().find(|n| {
        !n.is_extra()
            && n.end_byte() <= ctx.offset
            && matches!(
                n.kind(),
                "method_invocation"
                    | "object_creation_expression"
                    | "field_access"
                    | "identifier"
                    | "this"
            )
    })?;

    let child_end = expr_child.end_byte();
    if child_end > ctx.source.len() {
        return None;
    }
    let after = &ctx.source[child_end..ctx.offset.min(ctx.source.len())];
    let after_trimmed = after.trim_start();

    if !after_trimmed.starts_with('.') {
        return None;
    }

    let receiver_expr = ctx.node_text(*expr_child).trim().to_string();
    if receiver_expr.is_empty() {
        return None;
    }

    let after_dot = after_trimmed[1..].trim_start();
    let member_prefix: String = after_dot
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();

    Some((
        CursorLocation::MemberAccess {
            receiver_semantic_type: None,
            receiver_type: None,
            member_prefix: member_prefix.clone(),
            receiver_expr,
            arguments: None,
        },
        member_prefix,
    ))
}
