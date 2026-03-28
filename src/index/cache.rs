use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tracing::{debug, warn};

use crate::index::IndexedArchiveData;
use crate::index::store::{
    ArtifactKind, ArtifactSource, ArtifactStore, LmdbIndexStore, shared_store_root,
};

pub fn cache_dir() -> Option<PathBuf> {
    shared_store_root()
}

fn shared_store() -> Option<&'static LmdbIndexStore> {
    static STORE: OnceLock<Option<LmdbIndexStore>> = OnceLock::new();

    STORE
        .get_or_init(|| {
            let root = shared_store_root()?;
            match LmdbIndexStore::open(&root) {
                Ok(store) => Some(store),
                Err(err) => {
                    warn!(path = %root.display(), error = %err, "failed to open index store");
                    None
                }
            }
        })
        .as_ref()
}

pub fn load_cached(source_path: &Path) -> Option<IndexedArchiveData> {
    let source = ArtifactSource::from_path(source_path, detect_artifact_kind(source_path)).ok()?;
    let store = shared_store()?;
    let loaded = store.load_artifact(&source).ok()?;
    if let Some(artifact) = loaded {
        debug!(
            path = %source_path.display(),
            kind = ?artifact.metadata.kind,
            class_count = artifact.data.classes.len(),
            module_count = artifact.data.modules.len(),
            "loaded artifact from LMDB index store"
        );
        return Some(artifact.data);
    }
    None
}

pub fn save_cache(source_path: &Path, data: &IndexedArchiveData) {
    let Some(store) = shared_store() else {
        return;
    };
    let Ok(source) = ArtifactSource::from_path(source_path, detect_artifact_kind(source_path))
    else {
        return;
    };

    if let Err(err) = store.store_artifact(&source, data) {
        warn!(
            path = %source_path.display(),
            error = %err,
            "failed to persist artifact into LMDB index store"
        );
    }
}

fn detect_artifact_kind(path: &Path) -> ArtifactKind {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("modules") => ArtifactKind::JdkModulesImage,
        Some("src.zip") => ArtifactKind::SourceZip,
        _ if path.extension().is_some_and(|ext| ext == "jar") => ArtifactKind::Jar,
        _ => ArtifactKind::Unknown,
    }
}
