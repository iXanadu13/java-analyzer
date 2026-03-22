use rayon::prelude::*;
use salsa::Setter;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tower_lsp::lsp_types::Url;
use tree_sitter::Tree;

use super::{ClassMetadata, ClassOrigin, NameTable};
use crate::salsa_db::ParseTreeOrigin;

pub struct SourceTextInput {
    pub uri: Arc<str>,
    pub language_id: Arc<str>,
    pub content: String,
    pub origin: ClassOrigin,
}

impl SourceTextInput {
    pub fn new(uri: Arc<str>, language_id: Arc<str>, content: String, origin: ClassOrigin) -> Self {
        Self {
            uri,
            language_id,
            content,
            origin,
        }
    }
}

pub enum PreparedSource {
    Java(PreparedJavaSource),
    Kotlin(PreparedKotlinSource),
}

impl PreparedSource {
    pub fn discover_internal_names(&self) -> Vec<Arc<str>> {
        match self {
            Self::Java(source) => source.discover_internal_names(),
            Self::Kotlin(source) => source.discover_internal_names(),
        }
    }

    pub fn extract_classes(&self, name_table: Option<Arc<NameTable>>) -> Vec<ClassMetadata> {
        match self {
            Self::Java(source) => source.extract_classes(name_table),
            Self::Kotlin(source) => source.extract_classes(),
        }
    }
}

pub struct PreparedJavaSource {
    content: String,
    origin: ClassOrigin,
    tree: Tree,
}

impl PreparedJavaSource {
    pub fn discover_internal_names(&self) -> Vec<Arc<str>> {
        crate::language::java::class_parser::discover_java_names_from_tree(
            self.content.as_str(),
            &self.tree,
        )
    }

    pub fn extract_classes(&self, name_table: Option<Arc<NameTable>>) -> Vec<ClassMetadata> {
        crate::language::java::class_parser::extract_java_classes_from_tree(
            self.content.as_str(),
            &self.tree,
            &self.origin,
            name_table,
            None,
        )
    }
}

pub struct PreparedKotlinSource {
    content: String,
    origin: ClassOrigin,
}

impl PreparedKotlinSource {
    pub fn discover_internal_names(&self) -> Vec<Arc<str>> {
        super::source::discover_kotlin_names(self.content.as_str())
    }

    pub fn extract_classes(&self) -> Vec<ClassMetadata> {
        super::source::parse_kotlin_source(self.content.as_str(), self.origin.clone())
    }
}

#[derive(Default)]
pub struct SourceParseSession {
    db: crate::salsa_db::Database,
    files: HashMap<Arc<str>, crate::salsa_db::SourceFile>,
}

impl SourceParseSession {
    pub fn prepare_source(&mut self, input: SourceTextInput) -> Option<PreparedSource> {
        match input.language_id.as_ref() {
            "java" => self.prepare_java_source(input).map(PreparedSource::Java),
            "kotlin" => Some(PreparedSource::Kotlin(PreparedKotlinSource {
                content: input.content,
                origin: input.origin,
            })),
            _ => None,
        }
    }

    fn prepare_java_source(&mut self, input: SourceTextInput) -> Option<PreparedJavaSource> {
        let file = self.get_or_create_source_file(
            Arc::clone(&input.uri),
            input.content.as_str(),
            Arc::clone(&input.language_id),
        )?;
        let tree = crate::salsa_queries::parse::parse_tree(&self.db, file)?;

        Some(PreparedJavaSource {
            content: input.content,
            origin: input.origin,
            tree,
        })
    }

    fn get_or_create_source_file(
        &mut self,
        uri: Arc<str>,
        content: &str,
        language_id: Arc<str>,
    ) -> Option<crate::salsa_db::SourceFile> {
        let url = Url::parse(uri.as_ref()).ok()?;

        if let Some(file) = self.files.get(&uri).copied() {
            if file.content(&self.db).as_str() != content {
                file.set_content(&mut self.db).to(content.to_owned());
            }
            if file.language_id(&self.db).as_ref() != language_id.as_ref() {
                file.set_language_id(&mut self.db).to(language_id);
            }
            return Some(file);
        }

        let file = crate::salsa_db::SourceFile::new(
            &self.db,
            crate::salsa_db::FileId::new(url),
            content.to_owned(),
            language_id,
        );
        self.files.insert(uri, file);
        Some(file)
    }

    pub fn prepare_sources(&mut self, inputs: Vec<SourceTextInput>) -> Vec<PreparedSource> {
        inputs
            .into_iter()
            .filter_map(|input| self.prepare_source(input))
            .collect()
    }

    pub fn prune_sources(&mut self, keep_uris: &HashSet<Arc<str>>) {
        let stale_uris = self
            .files
            .keys()
            .filter(|uri| !keep_uris.contains(*uri))
            .cloned()
            .collect::<Vec<_>>();

        for uri in stale_uris {
            let Some(file) = self.files.remove(&uri) else {
                continue;
            };
            let file_id = file.file_id(&self.db).clone();
            self.db.remove_parse_tree(&file_id);
            self.db.remove_class_extraction(&file_id);
        }
    }

    pub fn parse_tree_origin_for_uri(&self, uri: &str) -> Option<ParseTreeOrigin> {
        let file = self.files.get(uri)?;
        let file_id = file.file_id(&self.db);
        self.db
            .cached_parse_tree(&file_id)
            .map(|snapshot| snapshot.origin)
    }
}

pub fn prepare_source_inputs(inputs: Vec<SourceTextInput>) -> Vec<PreparedSource> {
    inputs
        .into_par_iter()
        .map_init(SourceParseSession::default, |session, input| {
            session.prepare_source(input)
        })
        .flatten()
        .collect()
}

pub fn parse_java_source_text(
    source: &str,
    origin: ClassOrigin,
    name_table: Option<Arc<NameTable>>,
) -> Vec<ClassMetadata> {
    let uri = source_uri_for_origin(&origin, "java");
    let mut session = SourceParseSession::default();
    let Some(PreparedSource::Java(prepared)) = session.prepare_source(SourceTextInput::new(
        uri,
        Arc::from("java"),
        source.to_owned(),
        origin,
    )) else {
        return vec![];
    };

    prepared.extract_classes(name_table)
}

pub fn discover_java_names_text(source: &str) -> Vec<Arc<str>> {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    let uri: Arc<str> = Arc::from(
        format!(
            "file:///__java_analyzer__/index/source-{:#016x}.java",
            hasher.finish()
        )
        .as_str(),
    );
    let mut session = SourceParseSession::default();
    let Some(PreparedSource::Java(prepared)) = session.prepare_source(SourceTextInput::new(
        uri,
        Arc::from("java"),
        source.to_owned(),
        ClassOrigin::Unknown,
    )) else {
        return vec![];
    };

    prepared.discover_internal_names()
}

pub fn source_uri_for_origin(origin: &ClassOrigin, language_id: &str) -> Arc<str> {
    if let ClassOrigin::SourceFile(uri) = origin {
        return Arc::clone(uri);
    }

    let mut hasher = DefaultHasher::new();
    origin.hash(&mut hasher);
    let ext = match language_id {
        "java" => "java",
        "kotlin" => "kt",
        _ => "txt",
    };

    Arc::from(
        format!(
            "file:///__java_analyzer__/index/{:016x}.{}",
            hasher.finish(),
            ext
        )
        .as_str(),
    )
}
