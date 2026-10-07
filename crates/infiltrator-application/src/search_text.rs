//! Map case-insensitive matches back to original UTF-8 text, without changing user bytes.
use infiltrator_contract::search_text::SearchTextRun;
use unicode_segmentation::UnicodeSegmentation;

pub fn project_text_runs(value: &str, query: &str) -> Vec<SearchTextRun> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() || value.is_empty() {
        return vec![SearchTextRun {
            text: value.into(),
            highlighted: false,
        }];
    }
    let mut lowered = String::new();
    let mut boundaries = Vec::new();
    for (original_start, grapheme) in value.grapheme_indices(true) {
        let original_end = original_start + grapheme.len();
        for character in grapheme.chars() {
            for lower in character.to_lowercase() {
                let start = lowered.len();
                lowered.push(lower);
                boundaries.push((start, lowered.len(), original_start, original_end));
            }
        }
    }
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for (start, matched) in lowered.match_indices(&needle) {
        let end = start + matched.len();
        let original_start = boundaries[boundaries.partition_point(|entry| entry.1 <= start)].2;
        let original_end = boundaries[boundaries.partition_point(|entry| entry.0 < end) - 1].3;
        if let Some(last) = ranges.last_mut()
            && last.1 >= original_start
        {
            last.1 = last.1.max(original_end);
        } else {
            ranges.push((original_start, original_end));
        }
    }
    let mut runs = Vec::new();
    let mut offset = 0;
    for (start, end) in ranges {
        if start > offset {
            runs.push(SearchTextRun {
                text: value[offset..start].into(),
                highlighted: false,
            });
        }
        runs.push(SearchTextRun {
            text: value[start..end].into(),
            highlighted: true,
        });
        offset = end;
    }
    if offset < value.len() {
        runs.push(SearchTextRun {
            text: value[offset..].into(),
            highlighted: false,
        });
    }
    runs
}

/// Expand regex byte ranges to whole graphemes before producing immutable native text runs.
pub fn project_match_ranges(
    value: &str,
    ranges: impl Iterator<Item = (usize, usize)>,
) -> Vec<SearchTextRun> {
    let graphemes: Vec<_> = value
        .grapheme_indices(true)
        .map(|(start, text)| (start, start + text.len()))
        .collect();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in ranges {
        if start >= end || end > value.len() {
            continue;
        }
        let first = graphemes.partition_point(|(_, boundary)| *boundary <= start);
        let last = graphemes.partition_point(|(boundary, _)| *boundary < end);
        let (start, end) = (graphemes[first].0, graphemes[last - 1].1);
        if let Some(previous) = merged.last_mut()
            && previous.1 >= start
        {
            previous.1 = previous.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    let mut result = Vec::new();
    let mut offset = 0;
    for (start, end) in merged {
        if offset < start {
            result.push(SearchTextRun {
                text: value[offset..start].into(),
                highlighted: false,
            });
        }
        result.push(SearchTextRun {
            text: value[start..end].into(),
            highlighted: true,
        });
        offset = end;
    }
    if offset < value.len() || result.is_empty() {
        result.push(SearchTextRun {
            text: value[offset..].into(),
            highlighted: false,
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::project_text_runs;
    #[test]
    fn matching_retains_original_unicode_and_case_expansion_boundaries() {
        let runs = project_text_runs("节点İHOST/İHost", "i");
        assert_eq!(
            runs.iter().map(|run| run.text.as_str()).collect::<String>(),
            "节点İHOST/İHost"
        );
        assert_eq!(
            runs.iter()
                .filter(|run| run.highlighted)
                .map(|run| run.text.as_str())
                .collect::<Vec<_>>(),
            ["İ", "İ"]
        );
        let runs = project_text_runs("API.Example/API", "api");
        assert_eq!(
            runs.iter()
                .filter(|run| run.highlighted)
                .map(|run| run.text.as_str())
                .collect::<Vec<_>>(),
            ["API", "API"]
        );
        assert!(
            project_text_runs("unchanged", "absent")
                .iter()
                .all(|run| !run.highlighted)
        );
        assert!(
            project_text_runs("unchanged", "")
                .iter()
                .all(|run| !run.highlighted)
        );
    }
    #[test]
    fn highlighting_keeps_combining_marks_and_emoji_sequences_in_one_run() {
        let runs = project_text_runs("e\u{301} host", "e");
        assert_eq!(runs[0].text, "e\u{301}");
        assert!(runs[0].highlighted);
        let family = "👩‍👩‍👧‍👦";
        let runs = project_text_runs(family, "👩");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text, family);
        assert!(runs[0].highlighted);
    }
}
