use rayon::prelude::*;
use salsa::Setter;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tower_lsp::lsp_types::Url;
use tree_sitter::Tree;

use super::{ClassMetadata, ClassOrigin, NameTable, SourceDeclarationBatch};
use crate::salsa_db::ParseTreeOrigin;

pub struct SourceTextInput {
    pub uri: Arc<str>,
    pub language_id: Arc<str>,
    pub content: Arc<str>,
    pub origin: ClassOrigin,
}

impl SourceTextInput {
    pub fn new(
        uri: Arc<str>,
        language_id: Arc<str>,
        content: impl Into<Arc<str>>,
        origin: ClassOrigin,
    ) -> Self {
        Self {
            uri,
            language_id,
            content: content.into(),
            origin,
        }
    }
}

pub struct PreparedSource {
    language_id: Arc<str>,
    content: Arc<str>,
    origin: ClassOrigin,
    tree: Option<Tree>,
}

impl PreparedSource {
    pub fn discover_internal_names(&self) -> Vec<Arc<str>> {
        crate::language::lookup_language(self.language_id.as_ref())
            .map(|language| {
                language.discover_internal_names(self.content.as_ref(), self.tree.as_ref())
            })
            .unwrap_or_default()
    }

    pub fn extract_classes(&self, name_table: Option<Arc<NameTable>>) -> Vec<ClassMetadata> {
        crate::language::lookup_language(self.language_id.as_ref())
            .map(|language| {
                language.extract_classes_from_source(
                    self.content.as_ref(),
                    &self.origin,
                    self.tree.as_ref(),
                    name_table,
                    None,
                )
            })
            .unwrap_or_default()
    }

    pub fn extract_index_data(
        &self,
        name_table: Option<Arc<NameTable>>,
    ) -> (Vec<ClassMetadata>, Option<SourceDeclarationBatch>) {
        let classes = self.extract_classes(name_table.clone());
        let declarations = if self.language_id.as_ref() == "java" {
            self.tree.as_ref().map(|tree| {
                crate::language::java::class_parser::extract_java_declarations_from_tree(
                    self.content.as_ref(),
                    tree,
                    &self.origin,
                    name_table,
                    None,
                )
            })
        } else {
            None
        };

        (classes, declarations)
    }

    pub fn origin(&self) -> &ClassOrigin {
        &self.origin
    }
}

#[derive(Default)]
pub struct SourceParseSession {
    db: crate::salsa_db::Database,
    files: HashMap<Arc<str>, crate::salsa_db::SourceFile>,
}

impl SourceParseSession {
    pub fn prepare_source(&mut self, input: SourceTextInput) -> Option<PreparedSource> {
        crate::language::lookup_language(input.language_id.as_ref())?;
        let file = self.get_or_create_source_file(
            Arc::clone(&input.uri),
            input.content.as_ref(),
            Arc::clone(&input.language_id),
        )?;
        let tree = crate::salsa_queries::parse::parse_tree(&self.db, file);

        Some(PreparedSource {
            language_id: input.language_id,
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
    let Some(prepared) = session.prepare_source(SourceTextInput::new(
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
    let Some(prepared) = session.prepare_source(SourceTextInput::new(
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
    let ext = crate::language::lookup_language(language_id)
        .and_then(|language| language.file_extensions().first().copied())
        .unwrap_or("txt");

    Arc::from(
        format!(
            "file:///__java_analyzer__/index/{:016x}.{}",
            hasher.finish(),
            ext
        )
        .as_str(),
    )
}
