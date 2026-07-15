//! Benchmark the two phonetic-similarity methods: speed, accuracy, and a note
//! on hardware/memory. Run: `cargo run --release -p xtask -- bench <lang.g2p>`.

use std::time::Instant;

use g2p::{phonemize, similarity, Method};

/// Small labelled accuracy set: (anchor, near, far) IPA triples where the
/// "near" word should score MORE similar to the anchor than the "far" word.
/// Curated minimal/near pairs vs unrelated words.
const TRIPLES: &[(&str, &str, &str)] = &[
    // anchor    near (1-feature/1-phoneme away)   far (unrelated)
    ("pat", "bat", "dog"),
    ("pin", "bin", "sun"),
    ("cat", "cap", "run"),
    ("ship", "sheep", "dog"),
    ("man", "men", "cup"),
    ("light", "night", "boat"),
    ("tall", "ball", "fish"),
    ("code", "coat", "milk"),
    ("sing", "sink", "warm"),
    ("veal", "feel", "rock"),
    ("goat", "coat", "swim"),
    ("robe", "rope", "hand"),
];

fn stats(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    (mean, var.sqrt())
}

pub fn run(model_path: &str) {
    let bytes = std::fs::read(model_path).expect("read model");
    let model = g2p::Model::from_bytes(&bytes);

    // Build an IPA corpus by phonemizing a spread of English-ish words.
    // (Any model works; we just need varied IPA strings.)
    let words: Vec<&str> = "the of and to in is are was word words water place \
        made live where after back little only round man year came show every good \
        me give under name very through just form much great think say help low line \
        before turn cause same mean differ move right boy old too does tell sentence \
        set three want air well also play small end put home read hand port large spell"
        .split_whitespace()
        .collect();
    let ipa: Vec<String> = words.iter().map(|w| phonemize(&model, w)).collect();

    println!("== phonetic similarity benchmark ==");
    println!(
        "model: {model_path}  |  corpus: {} IPA strings\n",
        ipa.len()
    );

    // ---- SPEED ----
    // All ordered pairs, repeated to get a stable timing.
    let reps = 40;
    for method in [Method::Weighted, Method::Levenshtein] {
        let mut acc = 0.0f64; // prevent optimizing the loop away
        let mut count = 0u64;
        let t0 = Instant::now();
        for _ in 0..reps {
            for a in &ipa {
                for b in &ipa {
                    acc += similarity(a, b, method) as f64;
                    count += 1;
                }
            }
        }
        let dt = t0.elapsed();
        let ns_per = dt.as_nanos() as f64 / count as f64;
        let ops = count as f64 / dt.as_secs_f64();
        println!(
            "SPEED  {:<12}  {:>7.1} ns/op   {:>8.2} M ops/s/core   (checksum {:.0})",
            format!("{method:?}"),
            ns_per,
            ops / 1e6,
            acc
        );
    }

    // ---- ACCURACY ----
    // For each triple, does the method rank near > far? Report accuracy + margin.
    println!();
    for method in [Method::Weighted, Method::Levenshtein] {
        let mut correct = 0;
        let mut margins = Vec::new();
        for (anc, near, far) in TRIPLES {
            let ia = phonemize(&model, anc);
            let sn = similarity(&ia, &phonemize(&model, near), method) as f64;
            let sf = similarity(&ia, &phonemize(&model, far), method) as f64;
            if sn > sf {
                correct += 1;
            }
            margins.push(sn - sf);
        }
        let (mean, sd) = stats(&margins);
        println!(
            "ACCURACY {:<12} rank(near>far): {}/{}   mean margin {:+.3} (sd {:.3})",
            format!("{method:?}"),
            correct,
            TRIPLES.len(),
            mean,
            sd
        );
    }

    // ---- RESOLUTION (how graded the scores are) ----
    // Weighted should produce distinct near vs far scores; Levenshtein is coarser.
    println!();
    for method in [Method::Weighted, Method::Levenshtein] {
        let mut vals = Vec::new();
        for a in &ipa {
            for b in &ipa {
                if a != b {
                    vals.push(similarity(a, b, method) as f64);
                }
            }
        }
        let distinct = {
            let mut q: Vec<u64> = vals.iter().map(|v| (v * 1000.0) as u64).collect();
            q.sort_unstable();
            q.dedup();
            q.len()
        };
        let (mean, sd) = stats(&vals);
        println!(
            "RESOLUTION {:<12} distinct scores: {:>4}   mean sim {:.3} (sd {:.3})",
            format!("{method:?}"),
            distinct,
            mean,
            sd
        );
    }

    println!(
        "\nHARDWARE: single-thread, CPU only. Per comparison both allocate an\n\
         (n+1)x(m+1) f32 DP matrix + segment vectors (n,m = phonemes/word, ~<15),\n\
         so memory is O(n*m) words -> a few KB, freed immediately; no heap growth\n\
         across calls. Weighted adds a constant-time feature-table lookup per\n\
         substitution (no allocation). Both are embarrassingly parallel across pairs."
    );
}
