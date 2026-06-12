pub(super) fn top_similar<'a, I>(query: &str, options: I, max: usize) -> Vec<&'a str>
where
    I: IntoIterator<Item = &'a str>,
{
    let threshold = (query.chars().count() / 3).max(2);
    let mut scored: Vec<(usize, &str)> = options
        .into_iter()
        .map(|option| (levenshtein(query, option), option))
        .filter(|(distance, _)| *distance <= threshold)
        .collect();
    scored.sort_by_key(|(distance, option)| (*distance, option.len()));
    scored
        .into_iter()
        .take(max)
        .map(|(_, option)| option)
        .collect()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().flat_map(|c| c.to_lowercase()).collect();
    let b: Vec<char> = b.chars().flat_map(|c| c.to_lowercase()).collect();
    let m = a.len();
    let n = b.len();
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

#[cfg(test)]
mod tests {
    use super::top_similar;

    #[test]
    fn finds_close_identifier_matches() {
        let candidates = ["cell_type", "score", "volume"];
        let suggestions = top_similar("celle_type", candidates.iter().copied(), 3);
        assert!(suggestions.contains(&"cell_type"));
    }

    #[test]
    fn ignores_unrelated_identifiers() {
        let candidates = ["cell_type", "score", "volume"];
        let suggestions = top_similar("zz", candidates.iter().copied(), 3);
        assert!(suggestions.is_empty());
    }
}
