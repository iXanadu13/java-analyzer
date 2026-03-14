use crate::language::java::JavaContextExtractor;
use crate::language::java::utils::strip_sentinel;
use crate::semantic::CursorLocation;
use tree_sitter::Node;

use super::text::detect_new_keyword_before_cursor;
use super::utils::{cursor_truncated_text, detect_variable_name_position};

/// cursor_node is the block itself (cases 4, 9, 10, 11, 13, 14)
pub(super) fn handle_block_as_cursor(
    ctx: &JavaContextExtractor,
    block: Node,
) -> (CursorLocation, String) {
    // Find last named child ending at or before cursor
    let last_child = {
        let mut wc = block.walk();
        let mut last: Option<Node> = None;
        for child in block.named_children(&mut wc) {
            if child.end_byte() <= ctx.offset {
                last = Some(child);
            }
        }
        last
    };

    if let Some(child) = last_child
        && child.kind() == "ERROR"
    {
        return handle_error_as_last_block_child(ctx, child);
    }

    // No ERROR: check for variable name position (e.g. `List<String> |`)
    if let Some(var_loc) = detect_variable_name_position(ctx, block) {
        return var_loc;
    }

    // Empty block or cursor after complete statement → Expression
    (
        CursorLocation::Expression {
            prefix: String::new(),
        },
        String::new(),
    )
}

/// ERROR is the last child of block before cursor (cases 9, 11, 12, 13)
fn handle_error_as_last_block_child(
    ctx: &JavaContextExtractor,
    error_node: Node,
) -> (CursorLocation, String) {
    // Case 9: `a.put` → ERROR contains scoped_type_identifier
    {
        let mut wc = error_node.walk();
        for child in error_node.named_children(&mut wc) {
            if child.kind() == "scoped_type_identifier"
                && let Some(r) = scoped_type_to_member_access(ctx, child)
            {
                return r;
            }
        }
    }

    let before = &ctx.source[..ctx.offset.min(ctx.source.len())];

    // Case 12/cases with `new`: detect new keyword
    if let Some((class_prefix, expected_type)) = detect_new_keyword_before_cursor(before) {
        return (
            CursorLocation::ConstructorCall {
                class_prefix: class_prefix.clone(),
                expected_type,
            },
            class_prefix,
        );
    }

    // Trailing dot in ERROR context (shouldn't usually happen here, but as safety)
    // Case 11/13: incomplete assignment (`int x =`) → Expression
    (
        CursorLocation::Expression {
            prefix: String::new(),
        },
        String::new(),
    )
}

/// `a.put` parsed as scoped_type_identifier → MemberAccess{receiver="a", prefix="put"}
pub(super) fn scoped_type_to_member_access(
    ctx: &JavaContextExtractor,
    scoped: Node,
) -> Option<(CursorLocation, String)> {
    let mut wc = scoped.walk();
    let parts: Vec<Node> = scoped.named_children(&mut wc).collect();
    if parts.len() < 2 {
        return None;
    }
    let member_node = *parts.last()?;
    // dot is one byte before member_node
    let receiver_end = member_node.start_byte().saturating_sub(1);
    let receiver_start = scoped.start_byte();
    if receiver_end <= receiver_start {
        return None;
    }
    let receiver_expr = ctx.source[receiver_start..receiver_end].trim().to_string();
    if receiver_expr.is_empty() || receiver_expr.contains(' ') || receiver_expr.contains('\t') {
        return None;
    }

    let member_prefix = strip_sentinel(&cursor_truncated_text(ctx, member_node));

    if receiver_expr.is_empty() {
        return None;
    }

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
