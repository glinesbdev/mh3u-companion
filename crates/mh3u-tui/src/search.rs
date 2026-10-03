//! Fuzzy matching for the crafting search.
//!
//! A query is split into words; every word must match something about a piece (its name, type, skills or
//! materials). A word matches a text if it appears in it as a substring, or (for words of 3+ letters) as a
//! loose subsequence that is no more than twice as spread out as the word is long, so `rthlos` finds `Rathalos`.

/// Score `token` against `text` (both lowercase). Higher is better; `None` means no match.
pub fn score(token: &str, text: &str) -> Option<u32> {
    if token.is_empty() {
        return Some(0);
    }
    if let Some(i) = text.find(token) {
        let mut s = 1000;
        if text[..i].chars().next_back().is_none_or(|c| !c.is_alphanumeric()) {
            s += 200; // starts a word
        }
        if text.len() == token.len() {
            s += 300; // exact
        }
        return Some(s - text.len().min(100) as u32); // shorter texts rank higher
    }
    let len = token.chars().count();
    if len < 3 {
        return None;
    }
    let span = if token.is_ascii() && text.is_ascii() {
        tightest_span(text.as_bytes(), token.as_bytes())
    } else {
        tightest_span(&text.chars().collect::<Vec<_>>(), &token.chars().collect::<Vec<_>>())
    }?;
    (span <= len * 2).then(|| (400 * len / span) as u32)
}

/// The length of the shortest stretch of `text` that contains `tok` as a subsequence.
fn tightest_span<T: PartialEq>(text: &[T], tok: &[T]) -> Option<usize> {
    let mut tightest: Option<usize> = None;
    for start in (0..text.len()).filter(|&i| text[i] == tok[0]) {
        let (mut matched, mut end) = (1, start);
        for (j, c) in text.iter().enumerate().skip(start + 1) {
            if matched == tok.len() {
                break;
            }
            if *c == tok[matched] {
                matched += 1;
                end = j;
            }
        }
        if matched == tok.len() {
            let span = end - start + 1;
            tightest = Some(tightest.map_or(span, |t| t.min(span)));
        }
    }
    tightest
}

/// Score a query word against a short fixed label such as `male`. Only a whole-word match or a prefix of 3+ letters
/// counts, so `male` doesn't match `female`.
pub fn score_tag(token: &str, tag: &str) -> Option<u32> {
    if token == tag {
        Some(1300)
    } else if token.len() >= 3 && tag.starts_with(token) {
        Some(900)
    } else {
        None
    }
}

/// A query word that asks for a rarity: `3` or `r3`, for rarities 1 to 10.
pub fn rarity_token(token: &str) -> Option<u8> {
    let digits = token.strip_prefix('r').unwrap_or(token);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().filter(|n| (1..=10).contains(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_match_whole_words_or_long_prefixes() {
        assert!(score_tag("male", "male").is_some());
        assert_eq!(score_tag("male", "female"), None);
        assert!(score_tag("fem", "female").is_some());
        assert!(score_tag("blade", "blademaster").is_some());
        assert_eq!(score_tag("ma", "male"), None); // too short for a prefix
    }

    #[test]
    fn rarity_tokens() {
        assert_eq!(rarity_token("3"), Some(3));
        assert_eq!(rarity_token("r10"), Some(10));
        assert_eq!(rarity_token("11"), None); // could be part of a name like "Type 41"
        assert_eq!(rarity_token("0"), None);
        assert_eq!(rarity_token("r"), None);
        assert_eq!(rarity_token("rathalos"), None);
    }

    #[test]
    fn substring_beats_fuzzy() {
        assert!(score("helm", "alloy helm").unwrap() > score("hlm", "alloy helm").unwrap());
    }

    #[test]
    fn word_start_and_exact_rank_higher() {
        let exact = score("poison", "poison").unwrap();
        let word_start = score("poison", "poison res").unwrap();
        let mid_word = score("poison", "antipoison").unwrap();
        assert!(exact > word_start && word_start > mid_word);
    }

    #[test]
    fn subsequence_needs_three_letters_and_a_tight_span() {
        assert!(score("rthlos", "rathalos mail").is_some());
        assert_eq!(score("rt", "rathalos mail"), None); // too short for fuzzy
        assert_eq!(score("rathalos", "r   a   t   h   a   l   o   s"), None); // too spread out
        assert_eq!(score("xyz", "rathalos"), None);
    }

    #[test]
    fn empty_token_matches_everything() {
        assert_eq!(score("", "anything"), Some(0));
    }
}
