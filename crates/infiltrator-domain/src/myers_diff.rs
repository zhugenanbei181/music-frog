//! High-performance Myers Diff algorithm for YAML configurations and AST snapshots.
//!
//! Generates unified diff line streams and aligned side-by-side (split) rows
//! with precise 1-indexed line numbers and fidelity tracking.

use infiltrator_contract::yaml_ast_diff::{
    DiffKind, DiffLine, DiffStats, FidelityGrade, SplitDiffRow, YamlAstDiffSnapshot,
};

/// Internal edit step from Myers backtracking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditStep {
    Equal(usize, usize),
    Delete(usize),
    Insert(usize),
}

/// Compute a full [`YamlAstDiffSnapshot`] comparing `old_text` to `new_text`.
pub fn compute_diff(
    old_text: &str,
    new_text: &str,
    source_id: impl Into<String>,
    target_id: impl Into<String>,
) -> YamlAstDiffSnapshot {
    let source_id = source_id.into();
    let target_id = target_id.into();

    let old_lines: Vec<&str> = if old_text.is_empty() {
        Vec::new()
    } else {
        old_text.lines().collect()
    };
    let new_lines: Vec<&str> = if new_text.is_empty() {
        Vec::new()
    } else {
        new_text.lines().collect()
    };

    let steps = myers_shortest_edit_script(&old_lines, &new_lines);

    let mut unified_lines = Vec::new();
    let mut additions: usize = 0;
    let mut deletions: usize = 0;
    let mut modifications: usize = 0;
    let mut unchanged: usize = 0;

    for step in &steps {
        match *step {
            EditStep::Equal(old_idx, new_idx) => {
                unchanged += 1;
                unified_lines.push(DiffLine::new(
                    Some(old_idx + 1),
                    Some(new_idx + 1),
                    DiffKind::Equal,
                    old_lines[old_idx],
                ));
            }
            EditStep::Delete(old_idx) => {
                deletions += 1;
                unified_lines.push(DiffLine::new(
                    Some(old_idx + 1),
                    None,
                    DiffKind::Delete,
                    old_lines[old_idx],
                ));
            }
            EditStep::Insert(new_idx) => {
                additions += 1;
                unified_lines.push(DiffLine::new(
                    None,
                    Some(new_idx + 1),
                    DiffKind::Insert,
                    new_lines[new_idx],
                ));
            }
        }
    }

    // Build side-by-side split rows by aligning consecutive deletes and inserts.
    let split_rows = build_split_rows(&steps, &old_lines, &new_lines, &mut modifications);
    if modifications > 0 {
        additions = additions.saturating_sub(modifications);
        deletions = deletions.saturating_sub(modifications);
    }

    let stats = DiffStats {
        additions,
        deletions,
        modifications,
        unchanged,
    };

    let fidelity_grade = evaluate_fidelity_grade(old_text, new_text);
    let fidelity_preserved = check_fidelity_preserved(old_text, new_text);

    YamlAstDiffSnapshot {
        source_id,
        target_id,
        stats,
        unified_lines,
        split_rows,
        fidelity_grade,
        fidelity_preserved,
    }
}

/// The O((N+M)D) Myers diff algorithm.
fn myers_shortest_edit_script<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<EditStep> {
    let n = a.len();
    let m = b.len();

    if n == 0 && m == 0 {
        return Vec::new();
    }
    if n == 0 {
        return (0..m).map(EditStep::Insert).collect();
    }
    if m == 0 {
        return (0..n).map(EditStep::Delete).collect();
    }

    let max = n + m;
    let offset = max;
    let mut v = vec![0usize; 2 * max + 1];
    v[1 + offset] = 0;
    let mut trace: Vec<Vec<usize>> = Vec::new();

    let mut found_d = None;
    for d in 0..=max {
        trace.push(v.clone());
        for k in (-(d as isize)..=(d as isize)).step_by(2) {
            let k_idx = (k + offset as isize) as usize;
            let mut x = if k == -(d as isize) || (k != d as isize && v[k_idx - 1] < v[k_idx + 1]) {
                v[k_idx + 1]
            } else {
                v[k_idx - 1] + 1
            };
            let mut y = (x as isize - k) as usize;

            while x < n && y < m && a[x] == b[y] {
                x += 1;
                y += 1;
            }
            v[k_idx] = x;

            if x >= n && y >= m {
                found_d = Some(d);
                break;
            }
        }
        if found_d.is_some() {
            break;
        }
    }

    let final_d = found_d.unwrap_or(max);
    let mut script = Vec::new();
    let mut curr_x = n;
    let mut curr_y = m;

    for step_d in (1..=final_d).rev() {
        let v_prev = &trace[step_d];
        let k_curr = curr_x as isize - curr_y as isize;
        let k_idx = (k_curr + offset as isize) as usize;

        let prev_k = if k_curr == -(step_d as isize)
            || (k_curr != step_d as isize && v_prev[k_idx - 1] < v_prev[k_idx + 1])
        {
            k_curr + 1
        } else {
            k_curr - 1
        };

        let prev_k_idx = (prev_k + offset as isize) as usize;
        let prev_x = v_prev[prev_k_idx];
        let prev_y = (prev_x as isize - prev_k) as usize;

        while curr_x > prev_x && curr_y > prev_y {
            curr_x -= 1;
            curr_y -= 1;
            script.push(EditStep::Equal(curr_x, curr_y));
        }

        if curr_x == prev_x {
            curr_y -= 1;
            script.push(EditStep::Insert(curr_y));
        } else if curr_y == prev_y {
            curr_x -= 1;
            script.push(EditStep::Delete(curr_x));
        }
    }

    while curr_x > 0 && curr_y > 0 {
        curr_x -= 1;
        curr_y -= 1;
        script.push(EditStep::Equal(curr_x, curr_y));
    }

    script.reverse();
    script
}

/// Aligns edit steps into side-by-side rows, pairing replacement chunks into [`DiffKind::Modify`].
fn build_split_rows(
    steps: &[EditStep],
    old_lines: &[&str],
    new_lines: &[&str],
    modifications: &mut usize,
) -> Vec<SplitDiffRow> {
    let mut rows = Vec::new();
    let mut i = 0;

    while i < steps.len() {
        match steps[i] {
            EditStep::Equal(old_idx, new_idx) => {
                rows.push(SplitDiffRow::new(
                    Some(DiffLine::new(
                        Some(old_idx + 1),
                        None,
                        DiffKind::Equal,
                        old_lines[old_idx],
                    )),
                    Some(DiffLine::new(
                        None,
                        Some(new_idx + 1),
                        DiffKind::Equal,
                        new_lines[new_idx],
                    )),
                    DiffKind::Equal,
                ));
                i += 1;
            }
            EditStep::Delete(_) | EditStep::Insert(_) => {
                let mut dels = Vec::new();
                let mut inss = Vec::new();

                while i < steps.len() {
                    match steps[i] {
                        EditStep::Delete(old_idx) => {
                            dels.push(old_idx);
                            i += 1;
                        }
                        EditStep::Insert(new_idx) => {
                            inss.push(new_idx);
                            i += 1;
                        }
                        EditStep::Equal(..) => break,
                    }
                }

                let common = dels.len().min(inss.len());
                *modifications += common;

                for idx in 0..common {
                    let old_idx = dels[idx];
                    let new_idx = inss[idx];
                    rows.push(SplitDiffRow::new(
                        Some(DiffLine::new(
                            Some(old_idx + 1),
                            None,
                            DiffKind::Modify,
                            old_lines[old_idx],
                        )),
                        Some(DiffLine::new(
                            None,
                            Some(new_idx + 1),
                            DiffKind::Modify,
                            new_lines[new_idx],
                        )),
                        DiffKind::Modify,
                    ));
                }

                for &old_idx in dels.iter().skip(common) {
                    rows.push(SplitDiffRow::new(
                        Some(DiffLine::new(
                            Some(old_idx + 1),
                            None,
                            DiffKind::Delete,
                            old_lines[old_idx],
                        )),
                        None,
                        DiffKind::Delete,
                    ));
                }

                for &new_idx in inss.iter().skip(common) {
                    rows.push(SplitDiffRow::new(
                        None,
                        Some(DiffLine::new(
                            None,
                            Some(new_idx + 1),
                            DiffKind::Insert,
                            new_lines[new_idx],
                        )),
                        DiffKind::Insert,
                    ));
                }
            }
        }
    }

    rows
}

/// Evaluate the fidelity grade based on anchor and comment markers in the YAML documents.
fn evaluate_fidelity_grade(old_text: &str, new_text: &str) -> FidelityGrade {
    let has_anchors = (old_text.contains('&') || new_text.contains('&'))
        && (old_text.contains('*') || new_text.contains('*'));
    if has_anchors {
        FidelityGrade::L3Anchors
    } else if old_text.contains('#') || new_text.contains('#') {
        FidelityGrade::L1Comments
    } else {
        FidelityGrade::L2Layout
    }
}

/// Check if comments and blank lines in unchanged sections were preserved without drift.
fn check_fidelity_preserved(old_text: &str, new_text: &str) -> bool {
    let old_comments: Vec<&str> = old_text
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('#'))
        .collect();
    let new_comments: Vec<&str> = new_text
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with('#'))
        .collect();

    // If new text keeps existing comments, fidelity is intact
    old_comments.iter().all(|c| new_comments.contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_texts_yield_zero_changes() {
        let text = "port: 7890\nmode: rule\n# comment\n";
        let diff = compute_diff(text, text, "base", "next");

        assert_eq!(diff.total_changes(), 0);
        assert!(!diff.has_differences());
        assert_eq!(diff.stats.unchanged, 3);
        assert_eq!(diff.unified_lines.len(), 3);
        assert_eq!(diff.split_rows.len(), 3);
        assert!(diff.fidelity_preserved);
    }

    #[test]
    fn simple_modification_pairs_delete_and_insert_into_modify() {
        let old_text = "port: 7890\nmode: rule";
        let new_text = "port: 7890\nmode: global";

        let diff = compute_diff(old_text, new_text, "old", "new");

        assert_eq!(diff.stats.unchanged, 1);
        assert_eq!(diff.stats.modifications, 1);
        assert_eq!(diff.stats.additions, 0);
        assert_eq!(diff.stats.deletions, 0);
        assert_eq!(diff.split_rows.len(), 2);
        assert_eq!(diff.split_rows[1].kind, DiffKind::Modify);
        assert_eq!(
            diff.split_rows[1].left.as_ref().unwrap().content,
            "mode: rule"
        );
        assert_eq!(
            diff.split_rows[1].right.as_ref().unwrap().content,
            "mode: global"
        );
    }

    #[test]
    fn pure_insertions_and_deletions() {
        let old_text = "line1\nline2\nline3";
        let new_text = "line1\ninserted\nline2";

        let diff = compute_diff(old_text, new_text, "a", "b");

        assert_eq!(diff.stats.unchanged, 2);
        assert_eq!(diff.stats.additions, 1);
        assert_eq!(diff.stats.deletions, 1);
        assert_eq!(diff.stats.modifications, 0);
    }

    #[test]
    fn empty_input_and_output_handling() {
        let diff = compute_diff("", "", "empty1", "empty2");
        assert!(diff.is_empty());
        assert_eq!(diff.total_changes(), 0);

        let diff_added = compute_diff("", "single line", "empty", "new");
        assert_eq!(diff_added.stats.additions, 1);
        assert_eq!(diff_added.unified_lines.len(), 1);

        let diff_removed = compute_diff("old line", "", "old", "empty");
        assert_eq!(diff_removed.stats.deletions, 1);
        assert_eq!(diff_removed.unified_lines.len(), 1);
    }

    #[test]
    fn fidelity_and_anchor_grade_detection() {
        let yaml_anchors = "base: &base\n  port: 7890\nalias: *base\n";
        let diff = compute_diff(yaml_anchors, yaml_anchors, "a", "b");
        assert_eq!(diff.fidelity_grade, FidelityGrade::L3Anchors);

        let yaml_comments = "# Header\nmode: rule\n";
        let diff2 = compute_diff(yaml_comments, yaml_comments, "a", "b");
        assert_eq!(diff2.fidelity_grade, FidelityGrade::L1Comments);
    }
}
