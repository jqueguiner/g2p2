//! Request and response payloads for the JSON API.

use serde::{Deserialize, Serialize};

use crate::lang_detect::Detection;

/// Phonetic distance method selector, parsed case-insensitively from the wire.
#[derive(Deserialize, Default, Clone, Copy, Debug)]
#[serde(rename_all = "lowercase")]
pub enum MethodArg {
    Levenshtein,
    #[default]
    Weighted,
}

impl From<MethodArg> for g2p::Method {
    fn from(m: MethodArg) -> Self {
        match m {
            MethodArg::Levenshtein => g2p::Method::Levenshtein,
            MethodArg::Weighted => g2p::Method::Weighted,
        }
    }
}

fn default_true() -> bool {
    true
}

// ---- /g2p ----

#[derive(Deserialize)]
pub struct G2pRequest {
    /// Word or whitespace-separated sequence of words to phonemize.
    pub text: String,
    /// Whisper language code. Omit/`null` to auto-detect from `text`.
    #[serde(default)]
    pub lang: Option<String>,
    /// Spell integer numerals as words before phonemizing (e.g. `12` -> `douze`).
    #[serde(default = "default_true")]
    pub numbers: bool,
}

#[derive(Serialize)]
pub struct WordPhonemes {
    pub word: String,
    pub phonemes: String,
}

#[derive(Serialize)]
pub struct G2pResponse {
    /// The (possibly numeral-expanded) text that was phonemized.
    pub text: String,
    /// Whisper code actually used.
    pub lang: String,
    /// `true` when `lang` came from auto-detection rather than the request.
    pub detected: bool,
    /// Detection detail, present only when auto-detection ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detection: Option<Detection>,
    /// Whether numeral expansion was applied.
    pub numbers_expanded: bool,
    pub words: Vec<WordPhonemes>,
    /// All word phoneme strings joined with a single space.
    pub ipa: String,
}

// ---- /detect ----

#[derive(Deserialize)]
pub struct DetectRequest {
    pub text: String,
}

// ---- /similarity ----

#[derive(Deserialize)]
pub struct SimilarityRequest {
    pub a: String,
    pub b: String,
    /// When `true` (default), `a`/`b` are graphemes to phonemize first;
    /// when `false`, they are treated as raw IPA.
    #[serde(default = "default_true")]
    pub phonemize: bool,
    /// Language for phonemization. Ignored when `phonemize` is `false`.
    /// Omit to auto-detect from `a`.
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub method: MethodArg,
}

#[derive(Serialize)]
pub struct SimilarityResponse {
    pub a_ipa: String,
    pub b_ipa: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    pub similarity: f32,
    pub distance: f32,
}

// ---- /alternatives ----

#[derive(Deserialize)]
pub struct AlternativesRequest {
    /// Reference name/word to match against.
    pub query: String,
    /// Candidate names/words to rank by phonetic closeness to `query`.
    pub candidates: Vec<String>,
    /// Language for phonemization. Omit to auto-detect from `query`.
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default)]
    pub method: MethodArg,
    /// Keep only the top-K results. `0`/omitted returns all.
    #[serde(default)]
    pub top_k: usize,
    /// Drop candidates below this similarity (`0.0..=1.0`).
    #[serde(default)]
    pub min_similarity: f32,
}

#[derive(Serialize)]
pub struct Alternative {
    pub name: String,
    pub ipa: String,
    pub similarity: f32,
}

#[derive(Serialize)]
pub struct AlternativesResponse {
    pub query: String,
    pub query_ipa: String,
    pub lang: String,
    pub method: String,
    pub results: Vec<Alternative>,
}

// ---- /languages ----

#[derive(Serialize)]
pub struct LanguageInfo {
    pub whisper: String,
    pub iso: String,
    pub logographic: bool,
    pub model_available: bool,
    pub loaded: bool,
}
