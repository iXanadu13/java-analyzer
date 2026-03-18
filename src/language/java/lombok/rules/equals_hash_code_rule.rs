use std::sync::Arc;
use tree_sitter::Node;
use tree_sitter_utils::traversal::first_child_of_kind;

use crate::{
    index::{AnnotationSummary, FieldSummary, MethodParam, MethodParams, MethodSummary},
    language::java::{
        JavaContextExtractor,
        lombok::{
            config::LombokConfig,
            types::annotations,
            utils::{find_lombok_annotation, get_bool_param, get_string_array_param},
        },
        members::parse_annotations_in_node,
        synthetic::{
            SyntheticDefinition, SyntheticDefinitionKind, SyntheticInput, SyntheticMemberRule,
            SyntheticMemberSet, SyntheticOrigin,
        },
        type_ctx::SourceTypeCtx,
    },
};

pub struct EqualsAndHashCodeRule;

impl SyntheticMemberRule for EqualsAndHashCodeRule {
    fn synthesize(
        &self,
        input: &SyntheticInput<'_>,
        out: &mut SyntheticMemberSet,
        explicit_methods: &[MethodSummary],
        explicit_fields: &[FieldSummary],
    ) {
        // Only process class-like declarations
        if !matches!(
            input.decl.kind(),
            "class_declaration" | "enum_declaration" | "record_declaration"
        ) {
            return;
        }

        // Load Lombok configuration
        let config = LombokConfig::new();

        // Check for class-level @EqualsAndHashCode annotation
        let class_annotations = extract_class_annotations(input.decl, input.ctx, input.type_ctx);
        let class_equals_hash_code =
            find_lombok_annotation(&class_annotations, annotations::EQUALS_AND_HASH_CODE);

        if let Some(annotation) = class_equals_hash_code {
            // Generate equals(), hashCode(), and canEqual() methods
            generate_equals_and_hash_code_methods(
                input,
                annotation,
                explicit_fields,
                explicit_methods,
                &config,
                out,
            );
        }
    }
}

/// Extract annotations from the class declaration
fn extract_class_annotations(
    decl: Node,
    ctx: &JavaContextExtractor,
    type_ctx: &SourceTypeCtx,
) -> Vec<AnnotationSummary> {
    if let Some(modifiers) = first_child_of_kind(decl, "modifiers") {
        parse_annotations_in_node(ctx, modifiers, type_ctx)
    } else {
        Vec::new()
    }
}

/// Generate equals(), hashCode(), and canEqual() methods
fn generate_equals_and_hash_code_methods(
    input: &SyntheticInput<'_>,
    annotation: &AnnotationSummary,
    explicit_fields: &[FieldSummary],
    explicit_methods: &[MethodSummary],
    config: &LombokConfig,
    out: &mut SyntheticMemberSet,
) {
    // Check if equals() or hashCode() already exist
    let has_equals = has_method(explicit_methods, "equals", "(Ljava/lang/Object;)Z");
    let has_hash_code = has_method(explicit_methods, "hashCode", "()I");

    // If either exists, don't generate any methods (they must be in sync)
    if has_equals || has_hash_code {
        return;
    }

    // Parse annotation parameters
    let _call_super = get_bool_param(annotation, "callSuper", false);
    let _do_not_use_getters = get_bool_param(
        annotation,
        "doNotUseGetters",
        config
            .get_bool("lombok.equalsAndHashCode.doNotUseGetters")
            .unwrap_or(false),
    );
    let only_explicitly_included = get_bool_param(annotation, "onlyExplicitlyIncluded", false);

    // Get exclude and of parameters
    let exclude_fields = get_string_array_param(annotation, "exclude");
    let of_fields = get_string_array_param(annotation, "of");

    // Determine which fields to include
    let _fields_to_include = determine_fields_for_equals_hash_code(
        explicit_fields,
        &exclude_fields,
        &of_fields,
        only_explicitly_included,
    );

    // Generate equals() method
    generate_equals_method(input, out);

    // Generate hashCode() method
    generate_hash_code_method(out);

    // Generate canEqual() method (for proper inheritance support)
    generate_can_equal_method(input, out);
}

/// Generate the equals(Object other) method
fn generate_equals_method(_input: &SyntheticInput<'_>, out: &mut SyntheticMemberSet) {
    let method_name: Arc<str> = Arc::from("equals");
    let descriptor: Arc<str> = Arc::from("(Ljava/lang/Object;)Z");

    // Create parameter
    let param = MethodParam {
        descriptor: Arc::from("Ljava/lang/Object;"),
        name: Arc::from("other"),
        annotations: Vec::new(),
    };

    // Add to methods
    out.methods.push(MethodSummary {
        name: method_name.clone(),
        params: MethodParams { items: vec![param] },
        annotations: Vec::new(),
        access_flags: rust_asm::constants::ACC_PUBLIC,
        is_synthetic: false,
        generic_signature: None,
        return_type: Some(Arc::from("Z")),
    });

    // Add to definitions
    out.definitions.push(SyntheticDefinition {
        kind: SyntheticDefinitionKind::Method,
        name: method_name,
        descriptor: Some(descriptor),
        origin: SyntheticOrigin::LombokEquals,
    });
}

/// Generate the hashCode() method
fn generate_hash_code_method(out: &mut SyntheticMemberSet) {
    let method_name: Arc<str> = Arc::from("hashCode");
    let descriptor: Arc<str> = Arc::from("()I");

    // Add to methods
    out.methods.push(MethodSummary {
        name: method_name.clone(),
        params: MethodParams { items: Vec::new() },
        annotations: Vec::new(),
        access_flags: rust_asm::constants::ACC_PUBLIC,
        is_synthetic: false,
        generic_signature: None,
        return_type: Some(Arc::from("I")),
    });

    // Add to definitions
    out.definitions.push(SyntheticDefinition {
        kind: SyntheticDefinitionKind::Method,
        name: method_name,
        descriptor: Some(descriptor),
        origin: SyntheticOrigin::LombokHashCode,
    });
}

/// Generate the canEqual(Object other) helper method
fn generate_can_equal_method(_input: &SyntheticInput<'_>, out: &mut SyntheticMemberSet) {
    let method_name: Arc<str> = Arc::from("canEqual");
    let descriptor: Arc<str> = Arc::from("(Ljava/lang/Object;)Z");

    // Create parameter
    let param = MethodParam {
        descriptor: Arc::from("Ljava/lang/Object;"),
        name: Arc::from("other"),
        annotations: Vec::new(),
    };

    // Add to methods (protected visibility)
    out.methods.push(MethodSummary {
        name: method_name.clone(),
        params: MethodParams { items: vec![param] },
        annotations: Vec::new(),
        access_flags: rust_asm::constants::ACC_PROTECTED,
        is_synthetic: false,
        generic_signature: None,
        return_type: Some(Arc::from("Z")),
    });

    // Add to definitions
    out.definitions.push(SyntheticDefinition {
        kind: SyntheticDefinitionKind::Method,
        name: method_name,
        descriptor: Some(descriptor),
        origin: SyntheticOrigin::LombokEquals, // canEqual is part of equals contract
    });
}

/// Determine which fields should be included in equals/hashCode
fn determine_fields_for_equals_hash_code(
    explicit_fields: &[FieldSummary],
    exclude_fields: &[Arc<str>],
    of_fields: &[Arc<str>],
    only_explicitly_included: bool,
) -> Vec<Arc<str>> {
    let mut result = Vec::new();

    for field in explicit_fields {
        let field_name = field.name.as_ref();

        // Skip static fields
        if field.access_flags & rust_asm::constants::ACC_STATIC != 0 {
            continue;
        }

        // Skip transient fields
        if is_field_transient(field) {
            continue;
        }

        // Skip fields starting with $ (unless explicitly included)
        if field_name.starts_with('$') && of_fields.is_empty() && !only_explicitly_included {
            continue;
        }

        // If 'of' is specified, only include those fields
        if !of_fields.is_empty() {
            if of_fields.iter().any(|f| f.as_ref() == field_name) {
                result.push(field.name.clone());
            }
            continue;
        }

        // Skip excluded fields
        if exclude_fields.iter().any(|f| f.as_ref() == field_name) {
            continue;
        }

        // Check for @EqualsAndHashCode.Include and @EqualsAndHashCode.Exclude annotations
        let has_include = field.annotations.iter().any(|a| {
            a.internal_name.as_ref() == "lombok/EqualsAndHashCode$Include"
                || a.internal_name.as_ref() == "EqualsAndHashCode$Include"
        });

        let has_exclude = field.annotations.iter().any(|a| {
            a.internal_name.as_ref() == "lombok/EqualsAndHashCode$Exclude"
                || a.internal_name.as_ref() == "EqualsAndHashCode$Exclude"
        });

        if has_exclude {
            continue;
        }

        if only_explicitly_included {
            if has_include {
                result.push(field.name.clone());
            }
        } else {
            // Include by default (unless excluded)
            result.push(field.name.clone());
        }
    }

    result
}

/// Check if a method with the given name and descriptor already exists
fn has_method(methods: &[MethodSummary], name: &str, descriptor: &str) -> bool {
    // For equals() and hashCode(), check name and parameter count/types
    // This is more lenient than exact descriptor matching to handle return type variations
    if name == "equals" {
        return methods.iter().any(|m| {
            m.name.as_ref() == "equals"
                && m.params.len() == 1
                && (m.params.items[0].descriptor.as_ref() == "Ljava/lang/Object;"
                    || m.params.items[0].descriptor.as_ref() == "LObject;")
        });
    }

    if name == "hashCode" {
        return methods
            .iter()
            .any(|m| m.name.as_ref() == "hashCode" && m.params.is_empty());
    }

    // Fallback to exact descriptor matching
    methods
        .iter()
        .any(|method| method.name.as_ref() == name && method.desc().as_ref() == descriptor)
}

/// Check if a field is transient
fn is_field_transient(field: &FieldSummary) -> bool {
    (field.access_flags & rust_asm::constants::ACC_TRANSIENT) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_fields_all() {
        let fields = vec![
            create_test_field("name", false, false),
            create_test_field("age", false, false),
            create_test_field("CONSTANT", true, false), // static
            create_test_field("temp", false, true),     // transient
        ];

        let result = determine_fields_for_equals_hash_code(&fields, &[], &[], false);

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|f| f.as_ref() == "name"));
        assert!(result.iter().any(|f| f.as_ref() == "age"));
    }

    #[test]
    fn test_determine_fields_with_exclude() {
        let fields = vec![
            create_test_field("name", false, false),
            create_test_field("age", false, false),
            create_test_field("password", false, false),
        ];

        let exclude = vec![Arc::from("password")];
        let result = determine_fields_for_equals_hash_code(&fields, &exclude, &[], false);

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|f| f.as_ref() == "name"));
        assert!(result.iter().any(|f| f.as_ref() == "age"));
        assert!(!result.iter().any(|f| f.as_ref() == "password"));
    }

    #[test]
    fn test_determine_fields_with_of() {
        let fields = vec![
            create_test_field("name", false, false),
            create_test_field("age", false, false),
            create_test_field("email", false, false),
        ];

        let of = vec![Arc::from("name"), Arc::from("email")];
        let result = determine_fields_for_equals_hash_code(&fields, &[], &of, false);

        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|f| f.as_ref() == "name"));
        assert!(result.iter().any(|f| f.as_ref() == "email"));
        assert!(!result.iter().any(|f| f.as_ref() == "age"));
    }

    #[test]
    fn test_skips_dollar_fields() {
        let fields = vec![
            create_test_field("name", false, false),
            create_test_field("$internal", false, false),
        ];

        let result = determine_fields_for_equals_hash_code(&fields, &[], &[], false);

        assert_eq!(result.len(), 1);
        assert!(result.iter().any(|f| f.as_ref() == "name"));
        assert!(!result.iter().any(|f| f.as_ref() == "$internal"));
    }

    fn create_test_field(name: &str, is_static: bool, is_transient: bool) -> FieldSummary {
        let mut flags = 0;
        if is_static {
            flags |= rust_asm::constants::ACC_STATIC;
        }
        if is_transient {
            flags |= rust_asm::constants::ACC_TRANSIENT;
        }

        FieldSummary {
            name: Arc::from(name),
            descriptor: Arc::from("Ljava/lang/String;"),
            access_flags: flags,
            annotations: Vec::new(),
            is_synthetic: false,
            generic_signature: None,
        }
    }
}
