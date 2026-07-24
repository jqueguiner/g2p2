//! HTTP handlers. Every handler is thin: parse -> resolve language ->
//! call into the g2p2 core -> serialize.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use g2p::Model;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::ApiError;
use crate::lang_detect::{self, Detection};
use crate::state::AppState;
use crate::types::*;

type St = State<Arc<AppState>>;

/// `GET /health`
pub async fn health(State(st): St) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "models_available": st.available.len(),
        "models_loaded": st.loaded().len(),
        "default_lang": st.default_lang,
    }))
}

/// `GET /languages` — the full Whisper set with per-language model status.
pub async fn languages(State(st): St) -> Json<Vec<LanguageInfo>> {
    let loaded = st.loaded();
    let list = g2p::lang::LANGS
        .iter()
        .map(|l| LanguageInfo {
            whisper: l.whisper.to_string(),
            iso: l.iso.to_string(),
            logographic: l.logo,
            model_available: st.has_model(l.whisper),
            loaded: loaded.contains(l.whisper),
        })
        .collect();
    Json(list)
}

// ---- language resolution ----

/// Resolve the language to use: an explicit request code wins; otherwise
/// auto-detect, then fall back to the server default. Returns the chosen code,
/// the detection record (if detection ran), and whether it was auto-detected.
fn resolve_lang(
    st: &AppState,
    requested: &Option<String>,
    text: &str,
) -> Result<(String, Option<Detection>, bool), ApiError> {
    if let Some(code) = requested {
        if !st.has_model(code) {
            return Err(ApiError::no_model(code, &st.available));
        }
        return Ok((code.clone(), None, false));
    }
    let detection = lang_detect::detect(text);
    let chosen = detection
        .as_ref()
        .and_then(|d| d.lang.clone())
        .filter(|c| st.has_model(c))
        .unwrap_or_else(|| st.default_lang.clone());
    if !st.has_model(&chosen) {
        return Err(ApiError::bad_request(format!(
            "could not detect a supported language and default '{}' has no model",
            st.default_lang
        )));
    }
    Ok((chosen, detection, true))
}

/// Phonemize a name/phrase: phonemize each whitespace token and concatenate the
/// IPA with no separator, so phonetic comparison isn't polluted by space
/// "segments".
fn phonemize_name(model: &Model, s: &str) -> String {
    s.split_whitespace()
        .map(|w| g2p::phonemize(model, w))
        .collect::<Vec<_>>()
        .concat()
}

// ---- /g2p ----

#[derive(Deserialize)]
pub struct G2pQuery {
    text: String,
    lang: Option<String>,
    numbers: Option<bool>,
}

/// `GET /g2p?text=bonjour&lang=fr&numbers=true` — convenience wrapper.
pub async fn g2p_get(State(st): St, Query(q): Query<G2pQuery>) -> Result<Json<G2pResponse>, ApiError> {
    let req = G2pRequest {
        text: q.text,
        lang: q.lang,
        numbers: q.numbers.unwrap_or(true),
    };
    g2p_run(&st, req)
}

/// `POST /g2p` — phonemize a word or sequence of words.
pub async fn g2p_post(State(st): St, Json(req): Json<G2pRequest>) -> Result<Json<G2pResponse>, ApiError> {
    g2p_run(&st, req)
}

fn g2p_run(st: &AppState, req: G2pRequest) -> Result<Json<G2pResponse>, ApiError> {
    if req.text.trim().is_empty() {
        return Err(ApiError::bad_request("`text` is empty"));
    }
    let (lang, detection, detected) = resolve_lang(st, &req.lang, &req.text)?;
    let model = st.model(&lang)?;

    let text = if req.numbers {
        g2p::expand_numbers(&req.text, &lang)
    } else {
        req.text.clone()
    };

    let words: Vec<WordPhonemes> = text
        .split_whitespace()
        .map(|w| WordPhonemes {
            word: w.to_string(),
            phonemes: g2p::phonemize(&model, w),
        })
        .collect();

    let ipa = words
        .iter()
        .map(|w| w.phonemes.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    Ok(Json(G2pResponse {
        text,
        lang,
        detected,
        detection: if detected { detection } else { None },
        numbers_expanded: req.numbers,
        words,
        ipa,
    }))
}

// ---- /detect ----

#[derive(Deserialize)]
pub struct DetectQuery {
    text: String,
}

/// `GET /detect?text=...`
pub async fn detect_get(Query(q): Query<DetectQuery>) -> Result<Json<Detection>, ApiError> {
    detect_run(&q.text)
}

/// `POST /detect`
pub async fn detect_post(Json(req): Json<DetectRequest>) -> Result<Json<Detection>, ApiError> {
    detect_run(&req.text)
}

fn detect_run(text: &str) -> Result<Json<Detection>, ApiError> {
    if text.trim().is_empty() {
        return Err(ApiError::bad_request("`text` is empty"));
    }
    lang_detect::detect(text)
        .map(Json)
        .ok_or_else(|| ApiError::bad_request("could not detect language"))
}

// ---- /similarity ----

/// `POST /similarity` — phonetic similarity between two strings.
pub async fn similarity(State(st): St, Json(req): Json<SimilarityRequest>) -> Result<Json<SimilarityResponse>, ApiError> {
    let method: g2p::Method = req.method.into();
    let method_name = format!("{method:?}").to_lowercase();

    let (a_ipa, b_ipa, lang) = if req.phonemize {
        let (lang, _, _) = resolve_lang(&st, &req.lang, &req.a)?;
        let model = st.model(&lang)?;
        (
            phonemize_name(&model, &req.a),
            phonemize_name(&model, &req.b),
            Some(lang),
        )
    } else {
        (req.a.clone(), req.b.clone(), None)
    };

    let sim = g2p::similarity(&a_ipa, &b_ipa, method);
    Ok(Json(SimilarityResponse {
        a_ipa,
        b_ipa,
        method: method_name,
        lang,
        similarity: sim,
        distance: 1.0 - sim,
    }))
}

// ---- /alternatives ----

/// `POST /alternatives` — rank candidate names by phonetic closeness to `query`.
pub async fn alternatives(State(st): St, Json(req): Json<AlternativesRequest>) -> Result<Json<AlternativesResponse>, ApiError> {
    if req.query.trim().is_empty() {
        return Err(ApiError::bad_request("`query` is empty"));
    }
    if req.candidates.is_empty() {
        return Err(ApiError::bad_request("`candidates` is empty"));
    }
    let method: g2p::Method = req.method.into();
    let method_name = format!("{method:?}").to_lowercase();

    let (lang, _, _) = resolve_lang(&st, &req.lang, &req.query)?;
    let model = st.model(&lang)?;

    let query_ipa = phonemize_name(&model, &req.query);

    let mut results: Vec<Alternative> = req
        .candidates
        .iter()
        .map(|name| {
            let ipa = phonemize_name(&model, name);
            let similarity = g2p::similarity(&query_ipa, &ipa, method);
            Alternative {
                name: name.clone(),
                ipa,
                similarity,
            }
        })
        .filter(|a| a.similarity >= req.min_similarity)
        .collect();

    results.sort_by(|a, b| b.similarity.total_cmp(&a.similarity));
    if req.top_k > 0 {
        results.truncate(req.top_k);
    }

    Ok(Json(AlternativesResponse {
        query: req.query,
        query_ipa,
        lang,
        method: method_name,
        results,
    }))
}
