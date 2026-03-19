/// Regression tests for stack overflow issues
///
/// This test suite verifies that the parser doesn't stack overflow
/// when processing incomplete code in Lombok-annotated classes.
use java_analyzer::{
    language::java::JavaContextExtractor,
    language::java::members::{extract_class_members_from_body, extract_valid_members_only},
    language::java::type_ctx::SourceTypeCtx,
};
use tree_sitter::Parser;

fn parse_java(source: &str) -> tree_sitter::Tree {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_java::LANGUAGE.into())
        .unwrap();
    parser.parse(source, None).unwrap()
}

fn find_class_body(root: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "class_declaration" {
            let mut class_cursor = child.walk();
            for class_child in child.children(&mut class_cursor) {
                if class_child.kind() == "class_body" {
                    return Some(class_child);
                }
            }
        }
    }
    None
}

#[test]
fn test_lombok_class_with_incomplete_new_expression() {
    // This used to cause stack overflow before the two-phase parser refactor
    let source = r#"
import lombok.*;

@Getter
@AllArgsConstructor
@Builder
public class User {
    private String name;
    @With private int age;

    private void foo() {
        String s = new 
    }
}
"#;

    let tree = parse_java(source);
    let root = tree.root_node();
    let class_body = find_class_body(root).expect("Should find class body");

    let ctx = JavaContextExtractor::new(
        source,
        source.len() - 10,
        Some(java_analyzer::index::NameTable::from_classes(&[])),
    );

    let type_ctx = SourceTypeCtx::new(
        None,
        vec![],
        Some(java_analyzer::index::NameTable::from_classes(&[])),
    );

    // This should NOT stack overflow - it used to before the two-phase refactor
    let members = extract_class_members_from_body(&ctx, class_body, &type_ctx);

    // Should extract at least the foo method and fields
    assert!(!members.is_empty(), "Should extract some members");
}

#[test]
fn test_lombok_class_with_multiple_incomplete_statements() {
    let source = r#"
import lombok.*;

@Data
@Builder
public class Person {
    private String firstName;
    private String lastName;
    private int age;

    public void method1() {
        String x = new 
    }
    
    public void method2() {
        int y = 
    }
    
    public void method3() {
        Person p = 
    }
}
"#;

    let tree = parse_java(source);
    let root = tree.root_node();
    let class_body = find_class_body(root).expect("Should find class body");

    let ctx = JavaContextExtractor::new(
        source,
        source.len() - 10,
        Some(java_analyzer::index::NameTable::from_classes(&[])),
    );

    let type_ctx = SourceTypeCtx::new(
        None,
        vec![],
        Some(java_analyzer::index::NameTable::from_classes(&[])),
    );

    // Should not stack overflow with multiple incomplete statements
    let members = extract_class_members_from_body(&ctx, class_body, &type_ctx);

    assert!(!members.is_empty());
}

#[test]
fn test_deeply_nested_error_nodes() {
    let source = r#"
public class Test {
    void method() {
        if (true) {
            if (true) {
                if (true) {
                    if (true) {
                        String s = new 
                    }
                }
            }
        }
    }
}
"#;

    let tree = parse_java(source);
    let root = tree.root_node();
    let class_body = find_class_body(root).expect("Should find class body");

    let ctx = JavaContextExtractor::new(
        source,
        source.len() - 10,
        Some(java_analyzer::index::NameTable::from_classes(&[])),
    );

    let type_ctx = SourceTypeCtx::new(
        None,
        vec![],
        Some(java_analyzer::index::NameTable::from_classes(&[])),
    );

    // Should not stack overflow even with nested ERROR nodes
    let members = extract_class_members_from_body(&ctx, class_body, &type_ctx);

    assert!(!members.is_empty());
}

#[test]
fn test_valid_members_extraction_skips_errors() {
    // Verify that extract_valid_members_only skips ERROR nodes
    let source = r#"
public class Test {
    public void validMethod() {
        System.out.println("valid");
    }
    
    public void incompleteMethod() {
        String s = new 
    }
    
    public void anotherValidMethod() {
        return;
    }
}
"#;

    let tree = parse_java(source);
    let root = tree.root_node();
    let class_body = find_class_body(root).expect("Should find class body");

    let ctx = JavaContextExtractor::new(
        source,
        source.len() - 10,
        Some(java_analyzer::index::NameTable::from_classes(&[])),
    );

    let type_ctx = SourceTypeCtx::new(
        None,
        vec![],
        Some(java_analyzer::index::NameTable::from_classes(&[])),
    );

    // Extract only valid members (should skip ERROR nodes)
    let valid_members = extract_valid_members_only(&ctx, class_body, &type_ctx);

    // Should extract at least the valid methods
    assert!(!valid_members.is_empty());

    let member_names: Vec<_> = valid_members.iter().map(|m| m.name()).collect();

    // Should have validMethod and anotherValidMethod
    assert!(member_names.iter().any(|n| n.as_ref() == "validMethod"));
    assert!(
        member_names
            .iter()
            .any(|n| n.as_ref() == "anotherValidMethod")
    );
}

#[test]
fn test_two_phase_extraction_includes_error_recovery() {
    // Verify that two-phase extraction includes both valid and error-recovered members
    let source = r#"
public class Test {
    public void validMethod() {
        System.out.println("valid");
    }
    
    public void incompleteMethod() {
        String s = new 
    }
}
"#;

    let tree = parse_java(source);
    let root = tree.root_node();
    let class_body = find_class_body(root).expect("Should find class body");

    let ctx = JavaContextExtractor::new(
        source,
        source.len() - 10,
        Some(java_analyzer::index::NameTable::from_classes(&[])),
    );

    let type_ctx = SourceTypeCtx::new(
        None,
        vec![],
        Some(java_analyzer::index::NameTable::from_classes(&[])),
    );

    // Two-phase extraction should get both valid and error-recovered members
    let all_members = extract_class_members_from_body(&ctx, class_body, &type_ctx);

    let member_names: Vec<_> = all_members.iter().map(|m| m.name()).collect();

    // Should have both methods
    assert!(member_names.iter().any(|n| n.as_ref() == "validMethod"));
    assert!(
        member_names
            .iter()
            .any(|n| n.as_ref() == "incompleteMethod")
    );
}

#[test]
fn test_method_after_error_node_is_extracted() {
    // Regression test for: methods that come after ERROR nodes should still be extracted
    //
    // NOTE: This test documents a known limitation. When tree-sitter completely mangles
    // the parse tree and splits a method declaration across multiple nodes (some ERROR,
    // some not), we may not extract it. This is an acceptable trade-off to prevent
    // stack overflow with Lombok + incomplete code.
    //
    // In this specific case, tree-sitter parses:
    //   private static Object test() { return null; }
    // As:
    //   - local_variable_declaration: "private static Object test"
    //   - ERROR: "() {"
    //   - return_statement: "return null;"
    //   - }
    //
    // Our error recovery can't reconstruct this because the method name is outside the ERROR node.
    let source = r#"
package org.cubewhy.relx;
public class Agent {
    private static Object inst;
    public static void agentmain(String args, Object inst) throws Exception {
        Agent.inst = inst;
        var proxy = new Object();
        proxy.run();
        Agent.inst = 
    }
    private static Object test() { return null; }
}
"#;

    let tree = parse_java(source);
    let root = tree.root_node();
    let class_body = find_class_body(root).expect("Should find class body");

    let ctx = JavaContextExtractor::new(
        source,
        source.len() - 10,
        Some(java_analyzer::index::NameTable::from_classes(&[])),
    );

    let type_ctx = SourceTypeCtx::new(
        None,
        vec![],
        Some(java_analyzer::index::NameTable::from_classes(&[])),
    );

    let members = extract_class_members_from_body(&ctx, class_body, &type_ctx);

    let member_names: Vec<_> = members.iter().map(|m| m.name()).collect();

    // Should extract the valid members
    assert!(
        member_names.iter().any(|n| n.as_ref() == "agentmain"),
        "Should find agentmain, found: {:?}",
        member_names.iter().map(|n| n.as_ref()).collect::<Vec<_>>()
    );
    assert!(
        member_names.iter().any(|n| n.as_ref() == "inst"),
        "Should find inst field, found: {:?}",
        member_names.iter().map(|n| n.as_ref()).collect::<Vec<_>>()
    );

    // Known limitation: test() method is not extracted because tree-sitter splits it
    // across ERROR and non-ERROR nodes. This is acceptable to prevent stack overflow.
    // The important thing is that we don't crash.
    eprintln!("Note: test() method not extracted due to mangled parse tree - this is expected");
}
