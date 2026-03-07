use std::sync::Arc;

/// Structured internal type names, using JVM-internal base names plus generic args and array dims.
/// - Objects: base_internal = "java/lang/String"
/// - Primitive types: base_internal = "int"
/// - Arrays: array_dims > 0
/// - With generics: args = [TypeName], rendered as "java/util/List<Ljava/lang/String;>"
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeName {
    pub base_internal: Arc<str>,
    pub args: Vec<TypeName>,
    pub array_dims: usize,
}

impl TypeName {
    pub fn new(base_internal: impl Into<Arc<str>>) -> Self {
        let raw: Arc<str> = base_internal.into();
        let mut base = raw.as_ref().trim();
        let mut dims = 0usize;
        while let Some(stripped) = base.strip_suffix("[]") {
            dims += 1;
            base = stripped.trim_end();
        }
        TypeName {
            base_internal: Arc::from(base),
            args: Vec::new(),
            array_dims: dims,
        }
    }

    pub fn with_args(base_internal: impl Into<Arc<str>>, args: Vec<TypeName>) -> Self {
        TypeName {
            base_internal: base_internal.into(),
            args,
            array_dims: 0,
        }
    }

    pub fn with_array_dims(mut self, dims: usize) -> Self {
        self.array_dims = dims;
        self
    }

    pub fn is_array(&self) -> bool {
        self.array_dims > 0
    }

    pub fn is_primitive(&self) -> bool {
        matches!(
            self.base_internal.as_ref(),
            "int" | "long" | "short" | "byte" | "char" | "float" | "double" | "boolean" | "void"
        )
    }

    pub fn has_generics(&self) -> bool {
        !self.args.is_empty()
    }

    /// Erase generic parameters and array dims for index lookup.
    pub fn erased_internal(&self) -> &str {
        &self.base_internal
    }

    /// Erase generics but keep array dims, e.g. "java/lang/String[]".
    pub fn erased_internal_with_arrays(&self) -> String {
        let mut s = self.base_internal.to_string();
        if self.array_dims > 0 {
            s.push_str(&"[]".repeat(self.array_dims));
        }
        s
    }

    pub fn contains_slash(&self) -> bool {
        self.base_internal.contains('/')
    }

    /// "java/lang/String[][]" → Some("java/lang/String[]")
    pub fn element_type(&self) -> Option<TypeName> {
        if self.array_dims == 0 {
            return None;
        }
        let mut next = self.clone();
        next.array_dims -= 1;
        Some(next)
    }

    /// "java/lang/String" → "java/lang/String[]"
    pub fn wrap_array(&self) -> TypeName {
        let mut next = self.clone();
        next.array_dims += 1;
        next
    }

    /// Internal style with generics, e.g. "java/util/List<Ljava/lang/String;>".
    pub fn to_internal_with_generics(&self) -> String {
        let base = self.base_internal.as_ref();
        let mut s = if self.args.is_empty() {
            base.to_string()
        } else {
            let arg_sigs: Vec<String> = self.args.iter().map(|a| a.to_jvm_signature()).collect();
            format!("{}<{}>", base, arg_sigs.join(""))
        };
        if self.array_dims > 0 {
            s.push_str(&"[]".repeat(self.array_dims));
        }
        s
    }

    /// JVM signature, e.g. "Ljava/util/List<Ljava/lang/String;>;" or "[I".
    pub fn to_jvm_signature(&self) -> String {
        let mut sig = match self.base_internal.as_ref() {
            "byte" => "B".to_string(),
            "char" => "C".to_string(),
            "double" => "D".to_string(),
            "float" => "F".to_string(),
            "int" => "I".to_string(),
            "long" => "J".to_string(),
            "short" => "S".to_string(),
            "boolean" => "Z".to_string(),
            "void" => "V".to_string(),
            "*" => "*".to_string(),
            "+" | "-" => {
                if let Some(inner) = self.args.first() {
                    format!("{}{}", self.base_internal, inner.to_jvm_signature())
                } else {
                    self.base_internal.to_string()
                }
            }
            base if base.contains('/') => {
                if self.args.is_empty() {
                    format!("L{};", base)
                } else {
                    let arg_sigs: Vec<String> =
                        self.args.iter().map(|a| a.to_jvm_signature()).collect();
                    format!("L{}<{}>;", base, arg_sigs.join(""))
                }
            }
            // Treat other identifiers as type variables (e.g. "T", "E").
            other => format!("T{};", other),
        };

        if self.array_dims > 0 {
            sig = format!("{}{}", "[".repeat(self.array_dims), sig);
        }
        sig
    }
}

impl From<&str> for TypeName {
    fn from(s: &str) -> Self {
        TypeName::new(s)
    }
}

impl From<String> for TypeName {
    fn from(s: String) -> Self {
        TypeName::new(s)
    }
}

impl From<Arc<str>> for TypeName {
    fn from(arc: Arc<str>) -> Self {
        TypeName::new(arc)
    }
}

impl std::fmt::Display for TypeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_internal_with_generics())
    }
}
