//! The chrome geometry a captured frame really painted.
//!
//! The space-utilization judge measures pixels, so the numbers it builds its
//! measurement box from must be the numbers that drew those pixels. Until now
//! it kept its own copy of the shell's page margin (30/16) and shell air (8/72)
//! and pinned the two sides against each other with a guard - a mirror, not a
//! single source: a skin, a zoom or a chrome decision that moved one of those
//! paddings would leave the judge measuring a box no surface had drawn.
//!
//! So the shell publishes what it used. At the end of a capture frame the host
//! reads the chrome facts back out of the live window and appends one
//! `CAPTURE_CHROME` line to that frame's marker file; the judge reads the line
//! per frame and never consults a remembered padding again. The line also
//! carries where the navigation stack really ends - measured by the sidebar,
//! not recomputed from row tokens in a second language - which is what finally
//! lets the rail axis tell eight painted rows from a chrome card parked under
//! them, in a short window as well as a tall one (`SPACEUTIL-006`,
//! `SPACEUTIL-007`).
//!
//! The key list is part of the exported page-structure document, so the writer
//! here, the Slint property that fills it and the Python reader all check
//! against one declared vocabulary rather than three matching guesses.

use crate::responsive_viewport::SidebarForm;
use serde::{Deserialize, Serialize};
use std::mem::replace;

/// The marker line prefix the capture host writes and the judge reads.
pub const CHROME_PREFIX: &str = "CAPTURE_CHROME";

/// Every key of the chrome line, in the order the line is written.
///
/// This is the wire vocabulary of a frame's chrome. The exporter publishes the
/// same list in `page-structure.json`, the writer below emits exactly it, and
/// the judge refuses a frame that does not carry all of it: a partially
/// described frame is not judged by remembered numbers, it is reported.
pub const CHROME_KEYS: [&str; 7] = [
    "nav_form",
    "nav_column_px",
    "page_margin_px",
    "shell_top_air_px",
    "shell_bottom_air_px",
    "dock_band_px",
    "scale_percent",
];

/// The chrome a single captured frame painted, in device-independent pixels.
///
/// Deliberately absent: where the navigation stack ends. The sidebar can be
/// asked, but b154's first attempt published 466px on a 1180x780 frame whose
/// last destination row really runs to 488px - a tail spacer's own offset is
/// not the stack's extent, and a number like that would accuse the pane's last
/// row of being a second content block. The rail axis therefore keeps its
/// current shape-blind reading, and the measured extent is registered on the
/// board (`SPACEUTIL-009`) instead of being guessed into a wire format.
///
/// Every field is consumed by the reader, and every one is a fact this frame
/// observed rather than a stage: `nav_form` is the typed navigation the shell
/// mounted, `nav_column_px` is the width that form really reserved (0 for a
/// bar), the three paddings are the page body's own box, `dock_band_px` is the
/// band the bar reserved, and `scale_percent` says the unit the rest were
/// measured in. Nothing here is kept "in case" a reader wants it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ShellChromeFacts {
    pub nav_form: SidebarForm,
    pub nav_column_px: u16,
    pub page_margin_px: u16,
    pub shell_top_air_px: u16,
    pub shell_bottom_air_px: u16,
    pub dock_band_px: u16,
    pub scale_percent: u16,
}

impl ShellChromeFacts {
    /// Whether this frame's chrome reserved a bottom band for a bar.
    ///
    /// A band belongs to a bar: the receipt is only self-consistent when the
    /// form that mounts one is the form that reserves it, which is the exact
    /// invariant `SPACEUTIL-006` had to be written for.
    pub const fn reserves_dock_band(self) -> bool {
        self.dock_band_px > 0
    }

    /// The marker line for this frame, one key per declared name, in order.
    pub fn to_marker_line(&self) -> String {
        format!(
            "{CHROME_PREFIX} nav_form={} nav_column_px={} page_margin_px={} \
             shell_top_air_px={} shell_bottom_air_px={} dock_band_px={} \
             scale_percent={}\n",
            self.nav_form.id(),
            self.nav_column_px,
            self.page_margin_px,
            self.shell_top_air_px,
            self.shell_bottom_air_px,
            self.dock_band_px,
            self.scale_percent,
        )
    }

    /// Read a chrome line back. `None` when it is not a chrome line, is missing
    /// a declared key, repeats one, or carries a value outside the vocabulary.
    ///
    /// This is the writer's exact inverse, so a frame the host could have
    /// written is a frame any consumer can re-read; nothing here infers a
    /// number the line does not carry.
    pub fn parse_marker_line(line: &str) -> Option<Self> {
        let line = line.trim_end_matches('\n');
        if line.starts_with("CAPTURE_READY") {
            return None;
        }
        let body = line.strip_prefix(CHROME_PREFIX)?;
        let mut seen = [false; CHROME_KEYS.len()];
        let mut fields = [0u16; CHROME_KEYS.len()];
        let mut nav_form: Option<SidebarForm> = None;
        for pair in body.split_whitespace() {
            let (key, value) = pair.split_once('=')?;
            let index = CHROME_KEYS.iter().position(|name| *name == key)?;
            if replace(&mut seen[index], true) {
                return None;
            }
            if key == "nav_form" {
                nav_form = Some(serde_json::from_str(&format!("\"{value}\"")).ok()?);
                continue;
            }
            fields[index] = value.parse().ok()?;
        }
        if !seen.iter().all(|found| *found) {
            return None;
        }
        Some(Self {
            nav_form: nav_form?,
            nav_column_px: fields[1],
            page_margin_px: fields[2],
            shell_top_air_px: fields[3],
            shell_bottom_air_px: fields[4],
            dock_band_px: fields[5],
            scale_percent: fields[6],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::page_structure::chrome_marker_keys;
    use crate::responsive_viewport::TIERS;

    fn sample(form: SidebarForm) -> ShellChromeFacts {
        ShellChromeFacts {
            nav_form: form,
            nav_column_px: form.width_px().unwrap_or(0),
            page_margin_px: 16,
            shell_top_air_px: 0,
            shell_bottom_air_px: 0,
            dock_band_px: if form == SidebarForm::BottomNav {
                72
            } else {
                0
            },
            scale_percent: 100,
        }
    }

    #[test]
    fn the_writer_and_the_reader_are_exact_inverses() {
        for form in [
            SidebarForm::BottomNav,
            SidebarForm::Rail,
            SidebarForm::Standard,
            SidebarForm::Wide,
        ] {
            let facts = sample(form);
            let line = facts.to_marker_line();
            assert_eq!(ShellChromeFacts::parse_marker_line(&line), Some(facts));
        }
    }

    #[test]
    fn the_written_line_carries_exactly_the_declared_keys() {
        let line = sample(SidebarForm::Rail).to_marker_line();
        let keys: Vec<&str> = line
            .split_whitespace()
            .skip(1)
            .map(|pair| pair.split('=').next().expect("key=value"))
            .collect();
        assert_eq!(keys, CHROME_KEYS.to_vec());
        // And the vocabulary the judge reads is this same declared list, not a
        // second copy somebody typed into Python.
        assert_eq!(chrome_marker_keys(), CHROME_KEYS.to_vec());
    }

    #[test]
    fn a_frame_that_does_not_describe_its_chrome_is_not_judged() {
        let full = sample(SidebarForm::Standard).to_marker_line();
        // The readiness line is not chrome and never parses as it.
        assert_eq!(
            ShellChromeFacts::parse_marker_line("CAPTURE_READY page=overview skin=dark\n"),
            None
        );
        // A dropped key is a half-described frame: refused, not defaulted.
        assert_eq!(
            ShellChromeFacts::parse_marker_line(&full.replace("dock_band_px=0 ", "")),
            None
        );
        // A repeated key is two answers to one question.
        assert_eq!(
            ShellChromeFacts::parse_marker_line(
                &full.replace("scale_percent=100", "scale_percent=100 page_margin_px=30")
            ),
            None
        );
        // A form outside the typed vocabulary is refused rather than guessed.
        assert_eq!(
            ShellChromeFacts::parse_marker_line(
                &full.replace("nav_form=standard", "nav_form=sidebar")
            ),
            None
        );
    }

    #[test]
    fn a_band_is_only_ever_reserved_by_the_form_that_mounts_a_bar() {
        // The invariant SPACEUTIL-006 exists for: a rail owns a column and no
        // band, a bar owns a band and no column. The receipt says both, so a
        // reader catches a shell that reserved a band for the wrong reason -
        // which is exactly what the page-density boolean used to do.
        for form in [
            SidebarForm::BottomNav,
            SidebarForm::Rail,
            SidebarForm::Standard,
            SidebarForm::Wide,
        ] {
            let facts = sample(form);
            assert_eq!(
                facts.reserves_dock_band(),
                form == SidebarForm::BottomNav,
                "{form:?} reserves a band exactly when it mounts a bar"
            );
        }
    }

    #[test]
    fn every_tier_can_describe_the_chrome_it_mounts() {
        for tier in TIERS {
            let form = tier.sidebar_form();
            let facts = sample(form);
            assert_eq!(
                facts.nav_column_px,
                tier.nav_column_px(),
                "{} reserves its exported column",
                tier.id()
            );
            assert_eq!(
                ShellChromeFacts::parse_marker_line(&facts.to_marker_line()),
                Some(facts)
            );
        }
    }
}
