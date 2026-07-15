//! Phonetic similarity between two IPA strings (as produced by [`crate::phonemize`]).
//!
//! Two methods:
//! - [`Method::Levenshtein`] — 0/1 substitution cost per differing phoneme. Fast, crude.
//! - [`Method::Weighted`] — substitution cost = articulatory feature distance
//!   (near sounds like p/b cost less than p/k). **Default, better.**
//!
//! Both align the two phoneme-segment sequences with a Needleman-Wunsch DP and
//! normalize to a 0..1 score. Zero dependencies.

/// Which distance to use. [`Method::Weighted`] is the default.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Method {
    /// 0/1 per differing phoneme. Fast, ignores phonetic closeness.
    Levenshtein,
    /// Articulatory feature distance per substitution. Default.
    #[default]
    Weighted,
}

const NF: usize = 10; // feature dimensions

/// Split an IPA string into phoneme segments: a base symbol plus its trailing
/// diacritics / length / tone marks; a tie bar (t͡ʃ) pulls in the next base.
pub fn segments(s: &str) -> Vec<Box<str>> {
    let mut out: Vec<Box<str>> = Vec::new();
    let mut cur = String::new();
    let mut pending_tie = false;
    for c in s.chars() {
        if cur.is_empty() {
            cur.push(c);
            pending_tie = is_tie(c);
        } else if is_mod(c) {
            cur.push(c);
            pending_tie |= is_tie(c);
        } else if pending_tie {
            cur.push(c); // base directly after a tie bar joins the affricate
            pending_tie = false;
        } else {
            out.push(cur.as_str().into());
            cur.clear();
            cur.push(c);
            pending_tie = is_tie(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur.as_str().into());
    }
    out
}

/// Phonetic distance in 0..1 (0 = identical). Segments internally.
pub fn distance(a: &str, b: &str, method: Method) -> f32 {
    let (sa, sb) = (segments(a), segments(b));
    if sa.is_empty() && sb.is_empty() {
        return 0.0;
    }
    let raw = edit(&sa, &sb, method);
    raw / sa.len().max(sb.len()) as f32
}

/// Phonetic similarity in 0..1 (1 = identical). `1 - distance`.
pub fn similarity(a: &str, b: &str, method: Method) -> f32 {
    (1.0 - distance(a, b, method)).max(0.0)
}

/// Needleman-Wunsch edit distance over phoneme segments (gap cost 1.0).
#[allow(clippy::needless_range_loop)] // 2D DP reads several cells by index
fn edit(a: &[Box<str>], b: &[Box<str>], method: Method) -> f32 {
    let (n, m) = (a.len(), b.len());
    let gap = 1.0;
    let mut d = vec![vec![0.0f32; m + 1]; n + 1];
    for i in 0..=n {
        d[i][0] = i as f32 * gap;
    }
    for j in 0..=m {
        d[0][j] = j as f32 * gap;
    }
    for i in 1..=n {
        for j in 1..=m {
            let sub = d[i - 1][j - 1] + sub_cost(&a[i - 1], &b[j - 1], method);
            let del = d[i - 1][j] + gap;
            let ins = d[i][j - 1] + gap;
            d[i][j] = sub.min(del).min(ins);
        }
    }
    d[n][m]
}

/// Substitution cost between two phoneme segments in 0..1.
fn sub_cost(a: &str, b: &str, method: Method) -> f32 {
    if a == b {
        return 0.0;
    }
    match method {
        Method::Levenshtein => 1.0,
        Method::Weighted => {
            let ca = a.chars().next().unwrap();
            let cb = b.chars().next().unwrap();
            match (features(ca), features(cb)) {
                (Some(fa), Some(fb)) => {
                    let l1: i32 = fa
                        .iter()
                        .zip(fb.iter())
                        .map(|(x, y)| (*x as i32 - *y as i32).abs())
                        .sum();
                    l1 as f32 / (2.0 * NF as f32) // max per-dim diff = 2
                }
                _ if ca == cb => 0.2, // same base, unknown features, diff diacritics
                _ => 1.0,             // unknown phoneme(s)
            }
        }
    }
}

#[inline]
fn is_tie(c: char) -> bool {
    matches!(c as u32, 0x0361 | 0x035C | 0x0362)
}

#[inline]
fn is_mod(c: char) -> bool {
    let u = c as u32;
    (0x0300..=0x036F).contains(&u)  // combining diacritics (incl. ties)
        || (0x02B0..=0x02FF).contains(&u) // modifier letters, length, tone letters
        || (0x2070..=0x209F).contains(&u) // super/subscripts
        || c.is_numeric()                 // tone digits
        || matches!(c, '.' | '\u{203F}')
}

/// Articulatory feature vector for a base IPA symbol, `None` if unknown.
/// Dims: syllabic, voiced, nasal, continuant, labial, coronal, dorsal, high, back, round.
/// Values in {-1, 0, 1}.
#[rustfmt::skip]
fn features(c: char) -> Option<[i8; NF]> {
    Some(match c {
        // stops
        'p' => [-1,-1,-1,-1, 1, 0, 0, 0, 0, 0],
        'b' => [-1, 1,-1,-1, 1, 0, 0, 0, 0, 0],
        't' => [-1,-1,-1,-1, 0, 1, 0, 0, 0, 0],
        'd' => [-1, 1,-1,-1, 0, 1, 0, 0, 0, 0],
        'k' => [-1,-1,-1,-1, 0, 0, 1, 1, 0, 0],
        'g' | 'ɡ' => [-1, 1,-1,-1, 0, 0, 1, 1, 0, 0],
        'q' => [-1,-1,-1,-1, 0, 0, 1, 0, 1, 0],
        'ʔ' => [-1,-1,-1,-1, 0, 0, 0, 0, 0, 0],
        // nasals
        'm' => [-1, 1, 1,-1, 1, 0, 0, 0, 0, 0],
        'ɱ' => [-1, 1, 1,-1, 1, 0, 0, 0, 0, 0],
        'n' => [-1, 1, 1,-1, 0, 1, 0, 0, 0, 0],
        'ŋ' => [-1, 1, 1,-1, 0, 0, 1, 1, 0, 0],
        'ɲ' => [-1, 1, 1,-1, 0, 1, 1, 1, 0, 0],
        'ɴ' => [-1, 1, 1,-1, 0, 0, 1, 0, 1, 0],
        // fricatives
        'f' => [-1,-1,-1, 1, 1, 0, 0, 0, 0, 0],
        'v' => [-1, 1,-1, 1, 1, 0, 0, 0, 0, 0],
        'θ' => [-1,-1,-1, 1, 0, 1, 0, 0, 0, 0],
        'ð' => [-1, 1,-1, 1, 0, 1, 0, 0, 0, 0],
        's' => [-1,-1,-1, 1, 0, 1, 0, 0, 0, 0],
        'z' => [-1, 1,-1, 1, 0, 1, 0, 0, 0, 0],
        'ʃ' => [-1,-1,-1, 1, 0, 1, 0, 1, 0, 0],
        'ʒ' => [-1, 1,-1, 1, 0, 1, 0, 1, 0, 0],
        'ç' => [-1,-1,-1, 1, 0, 1, 1, 1, 0, 0],
        'x' => [-1,-1,-1, 1, 0, 0, 1, 1, 0, 0],
        'ɣ' => [-1, 1,-1, 1, 0, 0, 1, 1, 0, 0],
        'χ' => [-1,-1,-1, 1, 0, 0, 1, 0, 1, 0],
        'ʁ' => [-1, 1,-1, 1, 0, 0, 1, 0, 1, 0],
        'ħ' => [-1,-1,-1, 1, 0, 0, 1, 0, 1, 0],
        'ʕ' => [-1, 1,-1, 1, 0, 0, 1, 0, 1, 0],
        'h' => [-1,-1,-1, 1, 0, 0, 0, 0, 0, 0],
        'ɸ' => [-1,-1,-1, 1, 1, 0, 0, 0, 0, 0],
        // approximants / liquids
        'l' => [-1, 1,-1, 1, 0, 1, 0, 0, 0, 0],
        'ɭ' => [-1, 1,-1, 1, 0, 1, 0, 0, 0, 0],
        'r' => [-1, 1,-1, 1, 0, 1, 0, 0, 0, 0],
        'ɾ' => [-1, 1,-1, 1, 0, 1, 0, 0, 0, 0],
        'ɹ' => [-1, 1,-1, 1, 0, 1, 0, 0, 0, 0],
        'w' => [-1, 1,-1, 1, 1, 0, 1, 1, 1, 1],
        'j' => [-1, 1,-1, 1, 0, 1, 1, 1, 0, 0],
        'ɥ' => [-1, 1,-1, 1, 1, 1, 1, 1, 0, 1],
        // vowels (high: 1 high, 0 mid, -1 low; back: -1 front, 0 central, 1 back)
        'i' => [ 1, 1,-1, 1, 0, 0, 0, 1,-1,-1],
        'y' => [ 1, 1,-1, 1, 0, 0, 0, 1,-1, 1],
        'ɪ' => [ 1, 1,-1, 1, 0, 0, 0, 1,-1,-1],
        'e' => [ 1, 1,-1, 1, 0, 0, 0, 0,-1,-1],
        'ø' => [ 1, 1,-1, 1, 0, 0, 0, 0,-1, 1],
        'ɛ' => [ 1, 1,-1, 1, 0, 0, 0,-1,-1,-1],
        'œ' => [ 1, 1,-1, 1, 0, 0, 0,-1,-1, 1],
        'æ' => [ 1, 1,-1, 1, 0, 0, 0,-1,-1,-1],
        'a' => [ 1, 1,-1, 1, 0, 0, 0,-1, 0,-1],
        'ə' => [ 1, 1,-1, 1, 0, 0, 0, 0, 0,-1],
        'ɐ' => [ 1, 1,-1, 1, 0, 0, 0,-1, 0,-1],
        'ɑ' => [ 1, 1,-1, 1, 0, 0, 0,-1, 1,-1],
        'ɔ' => [ 1, 1,-1, 1, 0, 0, 0,-1, 1, 1],
        'o' => [ 1, 1,-1, 1, 0, 0, 0, 0, 1, 1],
        'ʊ' => [ 1, 1,-1, 1, 0, 0, 0, 1, 1, 1],
        'u' => [ 1, 1,-1, 1, 0, 0, 0, 1, 1, 1],
        'ɯ' => [ 1, 1,-1, 1, 0, 0, 0, 1, 1,-1],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_is_one() {
        assert_eq!(similarity("naɪt", "naɪt", Method::Weighted), 1.0);
        assert_eq!(similarity("naɪt", "naɪt", Method::Levenshtein), 1.0);
    }

    #[test]
    fn segments_diphthong_and_affricate() {
        assert_eq!(segments("naɪt").len(), 4); // n a ɪ t
                                               // t + tie + ʃ -> one affricate segment
        assert_eq!(segments("t\u{0361}ʃa").len(), 2);
        // length mark attaches
        assert_eq!(segments("aː").len(), 1);
    }

    #[test]
    fn weighted_beats_levenshtein_for_near_sounds() {
        // p vs b differ only in voicing -> weighted keeps them similar...
        let w = similarity("pa", "ba", Method::Weighted);
        // ...but Levenshtein counts a full substitution
        let l = similarity("pa", "ba", Method::Levenshtein);
        assert!(w > l, "weighted {w} should exceed levenshtein {l}");
        assert!(w > 0.9); // only 1 feature of 10 differs on 1 of 2 phonemes
        assert!((l - 0.5).abs() < 1e-6); // 1 of 2 phonemes fully substituted
    }

    #[test]
    fn far_sounds_less_similar_than_near() {
        let near = similarity("pa", "ba", Method::Weighted); // voicing only
        let far = similarity("pa", "ka", Method::Weighted); // place + dorsal
        assert!(near > far);
    }

    #[test]
    fn empty_strings() {
        assert_eq!(similarity("", "", Method::Weighted), 1.0);
        assert!(similarity("a", "", Method::Weighted) < 1.0);
    }

    #[test]
    fn unknown_phonemes_fall_back() {
        // unknown symbols: equal -> 0 cost, different -> full cost
        assert_eq!(similarity("§", "§", Method::Weighted), 1.0);
        assert!(similarity("§", "¤", Method::Weighted) < 1.0);
    }

    #[test]
    fn default_method_is_weighted() {
        assert_eq!(Method::default(), Method::Weighted);
    }

    #[test]
    fn feature_table_all_symbols_resolve() {
        // Every symbol in the table must return features (exercises all arms),
        // and self-distance via a forced substitution stays small.
        let all = "pbtdkɡqʔmɱnŋɲɴfvθðszʃʒçxɣχʁħʕhɸlɭrɾɹwjɥiyɪeøɛœæaəɐɑɔoʊuɯ";
        for ch in all.chars() {
            assert!(features(ch).is_some(), "missing features for {ch}");
            // substituting a symbol for itself-with-length is cheap (same base)
            let s = similarity(&ch.to_string(), &format!("{ch}ː"), Method::Weighted);
            assert!(s > 0.0);
        }
        assert!(features('g').is_some()); // ASCII g alias
    }
}
