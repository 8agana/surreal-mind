//! Framework trait and common helpers.

use super::types::FrameworkOutput;

pub trait Framework {
    fn name(&self) -> &'static str;
    fn analyze(&self, input: &str) -> FrameworkOutput;
}

pub(crate) fn split_sentences(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in s.chars() {
        cur.push(ch);
        if (ch == '.' || ch == '!' || ch == '?') && !cur.trim().is_empty() {
            out.push(cur.trim().to_string());
            cur.clear();
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    if out.is_empty() && !s.trim().is_empty() {
        out.push(s.trim().to_string());
    }
    out
}

pub(crate) fn top_keywords(s: &str, n: usize) -> Vec<String> {
    use std::collections::HashMap;
    let mut freq: HashMap<String, usize> = HashMap::new();
    for w in s
        .split(|c: char| !c.is_alphanumeric())
        .map(|w| w.to_lowercase())
        .filter(|w| !w.is_empty())
    {
        if [
            "the", "and", "or", "a", "an", "to", "of", "in", "on", "for", "with", "is", "are",
            "be", "this", "that",
        ]
        .contains(&w.as_str())
        {
            continue;
        }
        *freq.entry(w).or_insert(0) += 1;
    }
    let mut v: Vec<(String, usize)> = freq.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    v.into_iter().take(n).map(|(k, _)| k).collect()
}
