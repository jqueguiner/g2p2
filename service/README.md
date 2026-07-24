# g2p2-server

HTTP REST service around the zero-dependency [g2p2](../README.md) grapheme-to-phoneme engine.

Phonemize a word or a sequence of words in any of the 100 Whisper languages, with
optional **language auto-detection**, spoken-**number** expansion, **phonetic
similarity**, and an **alternative-names** ranking endpoint (find the candidates
that *sound* closest to a query).

Built on `axum` + `tokio`. Depends on the local runtime crate `g2p2-core`
(path dep, `numbers` feature on). It is a **detached workspace** so its async deps
never leak into the repo's intentionally zero-dependency runtime workspace.

## Run

```bash
# 1. get the model blobs (from the g2p2 `models-v2` GitHub release)
./scripts/fetch-models.sh fr en zh          # a few languages
# ./scripts/fetch-models.sh                  # or all 100

# 2. run
cargo run --release
# G2P_MODELS_DIR=./models  G2P_DEFAULT_LANG=en  G2P_BIND=0.0.0.0:8080
```

| Env | Default | Meaning |
|-----|---------|---------|
| `G2P_MODELS_DIR`   | `models`        | dir of `<whisper>.g2p` blobs |
| `G2P_DEFAULT_LANG` | `en`            | fallback when detection fails |
| `G2P_BIND`         | `0.0.0.0:8080`  | listen address |

Models are **lazy-loaded** on first request per language and kept resident.

## Endpoints

### `GET /health`
```json
{ "status":"ok", "models_available":3, "models_loaded":1, "default_lang":"en" }
```

### `GET /languages`
All 100 Whisper languages with `iso`, `logographic`, `model_available`, `loaded`.

### `POST /g2p` (or `GET /g2p?text=...&lang=...&numbers=...`)
Phonemize a word or sequence. Omit `lang` to auto-detect.
```bash
curl -s localhost:8080/g2p -H 'content-type: application/json' \
  -d '{"text":"bonjour le monde 12","lang":"fr","numbers":true}'
```
```json
{
  "text":"bonjour le monde douze",
  "lang":"fr", "detected":false, "numbers_expanded":true,
  "words":[
    {"word":"bonjour","phonemes":"bɔ̃ʒuʁ"},
    {"word":"le","phonemes":"lə"},
    {"word":"monde","phonemes":"mɔ̃d"},
    {"word":"douze","phonemes":"duz"}
  ],
  "ipa":"bɔ̃ʒuʁ lə mɔ̃d duz"
}
```
With `lang` omitted, the response also carries a `detection` block
(`{lang, iso, script, confidence, reliable}`).

### `GET /detect?text=...`  ·  `POST /detect {text}`
Language detection (whatlang), mapped to a Whisper code.
```json
{ "lang":"fr", "iso":"fra", "script":"Latin", "confidence":0.97, "reliable":true }
```

### `POST /similarity`
Phonetic similarity in `0..1`. Phonemizes both sides first by default; set
`phonemize:false` to compare raw IPA. `method` is `weighted` (default, articulatory
feature distance) or `levenshtein`.
```bash
curl -s localhost:8080/similarity -H 'content-type: application/json' \
  -d '{"a":"Caitlin","b":"Katelyn","lang":"en","method":"weighted"}'
```
```json
{ "a_ipa":"...", "b_ipa":"...", "method":"weighted", "lang":"en",
  "similarity":0.94, "distance":0.06 }
```

### `POST /alternatives`
Rank candidate names by how close they *sound* to `query`. Omit `lang` to detect.
```bash
curl -s localhost:8080/alternatives -H 'content-type: application/json' -d '{
  "query":"Caitlin",
  "candidates":["Kaitlyn","Katelynn","Caitlyn","Katherine","Kaylin"],
  "lang":"en", "method":"weighted", "top_k":3, "min_similarity":0.5
}'
```
```json
{
  "query":"Caitlin", "query_ipa":"...", "lang":"en", "method":"weighted",
  "results":[
    {"name":"Caitlyn","ipa":"...","similarity":0.98},
    {"name":"Kaitlyn","ipa":"...","similarity":0.95},
    {"name":"Katelynn","ipa":"...","similarity":0.88}
  ]
}
```

## Notes
- Logographic languages (`zh`, `ja`, `yue`) resolve from the lexicon tier; detection
  falls back to script when whatlang can't pin a trigram model.
- Numeral expansion uses the core `numbers` feature (`12` → `douze`), 120+ languages.
