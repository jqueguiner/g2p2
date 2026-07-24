//! Shared application state: the models directory, the set of available
//! `.g2p` blobs, and a lazily-populated in-memory cache of parsed models.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use g2p::Model;

use crate::error::ApiError;

/// Process-wide state, shared behind an `Arc` across all requests.
pub struct AppState {
    /// Directory holding `<whisper>.g2p` model blobs.
    pub models_dir: PathBuf,
    /// Whisper codes for which a `.g2p` file exists on disk (scanned at boot).
    pub available: BTreeSet<String>,
    /// Parsed models, loaded on first use and kept resident.
    cache: RwLock<HashMap<String, Arc<Model>>>,
    /// Fallback language when detection fails and none is requested.
    pub default_lang: String,
}

impl AppState {
    /// Scan `models_dir` for `*.g2p` files and build the available-language set.
    pub fn new(models_dir: PathBuf, default_lang: String) -> Self {
        let available = scan_models(&models_dir);
        Self {
            models_dir,
            available,
            cache: RwLock::new(HashMap::new()),
            default_lang,
        }
    }

    /// `true` if a model blob for this Whisper code is present on disk.
    pub fn has_model(&self, lang: &str) -> bool {
        self.available.contains(lang)
    }

    /// Get a parsed model for `lang`, loading and caching it on first request.
    pub fn model(&self, lang: &str) -> Result<Arc<Model>, ApiError> {
        if let Some(m) = self.cache.read().unwrap().get(lang) {
            return Ok(m.clone());
        }
        if !self.available.contains(lang) {
            return Err(ApiError::no_model(lang, &self.available));
        }
        let path = self.models_dir.join(format!("{lang}.g2p"));
        let bytes = std::fs::read(&path)
            .map_err(|e| ApiError::internal(format!("read {}: {e}", path.display())))?;
        // `Model::from_bytes` asserts on malformed input; treat a bad blob as a
        // 500 rather than crashing the worker thread.
        let model = std::panic::catch_unwind(|| Model::from_bytes(&bytes))
            .map_err(|_| ApiError::internal(format!("corrupt model blob: {lang}.g2p")))?;
        let arc = Arc::new(model);
        self.cache
            .write()
            .unwrap()
            .insert(lang.to_string(), arc.clone());
        Ok(arc)
    }

    /// Whisper codes currently resident in the in-memory cache.
    pub fn loaded(&self) -> BTreeSet<String> {
        self.cache.read().unwrap().keys().cloned().collect()
    }
}

fn scan_models(dir: &Path) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return set;
    };
    for entry in rd.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(code) = name.strip_suffix(".g2p") {
            set.insert(code.to_string());
        }
    }
    set
}
