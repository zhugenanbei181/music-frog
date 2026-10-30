//! The themed gradient washes and the elevation ladder (EFFECTS-030
//! #119-#121): stop hygiene inside the official eight-stop cap, wash
//! endpoints pinned to the palette tones, and one shadow authority per tier.
//! test-intent: behavior
use iced::Gradient;
use infiltrator_iced::view::theme::Elevation;
use infiltrator_iced::view::theme::skins::{AMOLED, DARK, FOREST, LIGHT};

fn linear_stops(gradient: &Gradient) -> Vec<f32> {
    match gradient {
        Gradient::Linear(linear) => linear
            .stops
            .iter()
            .flatten()
            .map(|stop| stop.offset)
            .collect(),
    }
}

fn first_stop_color(gradient: &Gradient) -> iced::Color {
    match gradient {
        Gradient::Linear(linear) => {
            linear
                .stops
                .iter()
                .flatten()
                .next()
                .expect("themed washes always carry stops")
                .color
        }
    }
}

#[test]
fn every_skin_washes_with_two_ordered_stops_inside_the_cap() {
    for tokens in [LIGHT, DARK, FOREST, AMOLED] {
        for wash in [tokens.hero_wash, tokens.control_wash] {
            let stops = linear_stops(&wash);
            assert_eq!(stops.len(), 2, "themed washes stay two-stop");
            assert!((stops[0] - 0.0).abs() < 1e-6);
            assert!((stops[1] - 1.0).abs() < 1e-6);
            assert!(stops.windows(2).all(|pair| pair[0] <= pair[1]));
        }
    }
}

#[test]
fn hero_wash_starts_at_the_sidebar_tone() {
    for tokens in [LIGHT, DARK, FOREST, AMOLED] {
        let top = first_stop_color(&tokens.hero_wash);
        assert_eq!(
            top, tokens.sidebar,
            "hero wash top stop is the sidebar tone"
        );
    }
}

#[test]
fn elevation_ladder_has_one_authority_per_tier() {
    for tokens in [LIGHT, DARK, FOREST, AMOLED] {
        let card = tokens.elevation_shadow(Elevation::Card);
        let lifted = tokens.elevation_shadow(Elevation::Lifted);
        let floating = tokens.elevation_shadow(Elevation::Floating);
        assert_ne!(card, floating, "resting and floating physics differ");
        assert_eq!(lifted, floating, "a lifted card borrows floating physics");
    }
}

#[test]
fn highlight_border_is_lighter_than_the_card_hairline() {
    for tokens in [LIGHT, DARK, FOREST, AMOLED] {
        let sum = |c: iced::Color| c.r + c.g + c.b;
        assert!(
            sum(tokens.highlight_border) > sum(tokens.card_border),
            "the highlight hairline lifts the card hairline toward white"
        );
    }
}
