//! g2p2-server — HTTP REST front end for the zero-dependency g2p2 engine.
//!
//! Endpoints:
//!   GET  /health                  liveness + model counts
//!   GET  /languages               all 100 Whisper langs + per-lang model status
//!   GET  /detect?text=            language detection (whatlang -> Whisper code)
//!   POST /detect                  { text }
//!   GET  /g2p?text=&lang=&numbers= phonemize (query form)
//!   POST /g2p                     { text, lang?, numbers? }
//!   POST /similarity              { a, b, phonemize?, lang?, method? }
//!   POST /alternatives            { query, candidates[], lang?, method?, top_k?, min_similarity? }
//!
//! Config via env:
//!   G2P_MODELS_DIR   directory of `<whisper>.g2p` blobs   (default: ./models)
//!   G2P_DEFAULT_LANG fallback language                    (default: en)
//!   G2P_BIND         listen address                       (default: 0.0.0.0:8080)

mod error;
mod handlers;
mod lang_detect;
mod state;
mod types;

use std::path::PathBuf;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=info".into()),
        )
        .init();

    let models_dir =
        PathBuf::from(std::env::var("G2P_MODELS_DIR").unwrap_or_else(|_| "models".into()));
    let default_lang = std::env::var("G2P_DEFAULT_LANG").unwrap_or_else(|_| "en".into());
    let bind = std::env::var("G2P_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into());

    let state = Arc::new(AppState::new(models_dir.clone(), default_lang.clone()));

    if state.available.is_empty() {
        tracing::warn!(
            dir = %models_dir.display(),
            "no .g2p models found — run scripts/fetch-models.sh to download them"
        );
    } else {
        tracing::info!(
            dir = %models_dir.display(),
            count = state.available.len(),
            "models available"
        );
    }

    let app = Router::new()
        .route("/health", get(handlers::health))
        .route("/languages", get(handlers::languages))
        .route("/detect", get(handlers::detect_get).post(handlers::detect_post))
        .route("/g2p", get(handlers::g2p_get).post(handlers::g2p_post))
        .route("/similarity", post(handlers::similarity))
        .route("/alternatives", post(handlers::alternatives))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .unwrap_or_else(|e| panic!("bind {bind}: {e}"));
    tracing::info!(%bind, "g2p2-server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown");
}
