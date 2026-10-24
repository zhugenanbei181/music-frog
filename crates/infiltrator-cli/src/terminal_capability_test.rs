use super::{ColorDepth, GraphicsProtocol, TerminalCapabilities, TerminalEnvironment, negotiate};

#[test]
fn colorterm_truecolor_selects_the_top_tier() {
    let environment = TerminalEnvironment {
        colorterm: Some("truecolor".to_string()),
        ..TerminalEnvironment::default()
    };
    assert_eq!(negotiate(&environment).color_depth, ColorDepth::TrueColor);
}

#[test]
fn term_256color_selects_the_palette_tier() {
    let environment = TerminalEnvironment {
        term: Some("xterm-256color".to_string()),
        ..TerminalEnvironment::default()
    };
    assert_eq!(negotiate(&environment).color_depth, ColorDepth::Ansi256);
}

#[test]
fn term_without_color_information_falls_back_to_sixteen_colors() {
    let environment = TerminalEnvironment {
        term: Some("dumb".to_string()),
        ..TerminalEnvironment::default()
    };
    let capabilities = negotiate(&environment);
    assert_eq!(capabilities.color_depth, ColorDepth::Ansi16);
    assert_eq!(capabilities.graphics_protocol, GraphicsProtocol::None);
}

#[test]
fn no_color_downgrades_to_the_lowest_tier() {
    let environment = TerminalEnvironment {
        colorterm: Some("truecolor".to_string()),
        no_color: Some("1".to_string()),
        ..TerminalEnvironment::default()
    };
    assert_eq!(negotiate(&environment).color_depth, ColorDepth::Ansi16);
}

#[test]
fn force_color_levels_override_the_terminal_report() {
    let reported = TerminalEnvironment {
        colorterm: Some("truecolor".to_string()),
        ..TerminalEnvironment::default()
    };
    let forced = |level: &str| TerminalEnvironment {
        force_color: Some(level.to_string()),
        ..reported.clone()
    };

    assert_eq!(negotiate(&forced("3")).color_depth, ColorDepth::TrueColor);
    assert_eq!(negotiate(&forced("2")).color_depth, ColorDepth::Ansi256);
    assert_eq!(negotiate(&forced("1")).color_depth, ColorDepth::Ansi16);
    assert_eq!(negotiate(&forced("0")).color_depth, ColorDepth::Ansi16);
}

#[test]
fn known_truecolor_terminals_are_detected() {
    for term in ["xterm-kitty", "alacritty", "foot", "ghostty", "contour"] {
        let environment = TerminalEnvironment {
            term: Some(term.to_string()),
            ..TerminalEnvironment::default()
        };
        assert_eq!(
            negotiate(&environment).color_depth,
            ColorDepth::TrueColor,
            "term {term} should report truecolor"
        );
    }
}

#[test]
fn windows_terminal_reports_truecolor() {
    let environment = TerminalEnvironment {
        wt_session: Some("session-1".to_string()),
        ..TerminalEnvironment::default()
    };
    assert_eq!(negotiate(&environment).color_depth, ColorDepth::TrueColor);
}

#[test]
fn kitty_is_detected_by_window_id_and_by_term() {
    let by_window = TerminalEnvironment {
        kitty_window_id: Some("1".to_string()),
        ..TerminalEnvironment::default()
    };
    assert_eq!(
        negotiate(&by_window).graphics_protocol,
        GraphicsProtocol::Kitty
    );

    let by_term = TerminalEnvironment {
        term: Some("xterm-kitty".to_string()),
        ..TerminalEnvironment::default()
    };
    assert_eq!(
        negotiate(&by_term).graphics_protocol,
        GraphicsProtocol::Kitty
    );
}

#[test]
fn sixel_is_detected_from_the_terminal_program_and_the_xterm_version() {
    let by_program = TerminalEnvironment {
        term_program: Some("WezTerm".to_string()),
        ..TerminalEnvironment::default()
    };
    assert_eq!(
        negotiate(&by_program).graphics_protocol,
        GraphicsProtocol::Sixel
    );

    let by_version = TerminalEnvironment {
        xterm_version: Some("XTerm(388) with sixel".to_string()),
        ..TerminalEnvironment::default()
    };
    assert_eq!(
        negotiate(&by_version).graphics_protocol,
        GraphicsProtocol::Sixel
    );
}

#[test]
fn kitty_wins_over_sixel_when_both_are_reported() {
    let environment = TerminalEnvironment {
        term: Some("xterm-kitty".to_string()),
        term_program: Some("WezTerm".to_string()),
        ..TerminalEnvironment::default()
    };
    assert_eq!(
        negotiate(&environment).graphics_protocol,
        GraphicsProtocol::Kitty
    );
}

#[test]
fn negotiation_is_a_pure_function_of_the_reported_signals() {
    let environment = TerminalEnvironment {
        term: Some("xterm-256color".to_string()),
        colorterm: Some("truecolor".to_string()),
        term_program: Some("WezTerm".to_string()),
        ..TerminalEnvironment::default()
    };
    let first = negotiate(&environment);
    let second = negotiate(&environment);
    assert_eq!(first, second);
    assert_eq!(
        first,
        TerminalCapabilities {
            color_depth: ColorDepth::TrueColor,
            graphics_protocol: GraphicsProtocol::Sixel,
            charset: crate::charset::CharsetProfile::Unicode,
        }
    );
}

#[test]
fn color_depth_reports_its_palette_size_and_label() {
    assert_eq!(ColorDepth::TrueColor.palette_size(), 16_777_216);
    assert_eq!(ColorDepth::Ansi256.palette_size(), 256);
    assert_eq!(ColorDepth::Ansi16.palette_size(), 16);
    assert_eq!(ColorDepth::TrueColor.label(), "truecolor");
    assert_eq!(ColorDepth::Ansi256.label(), "ansi256");
    assert_eq!(ColorDepth::Ansi16.label(), "ansi16");
}

#[test]
fn graphics_protocol_reports_its_label() {
    assert_eq!(GraphicsProtocol::Kitty.label(), "kitty");
    assert_eq!(GraphicsProtocol::Sixel.label(), "sixel");
    assert_eq!(GraphicsProtocol::None.label(), "none");
}
