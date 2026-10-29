use super::*;
use infiltrator_contract::subscription_quota::SubscriptionQuotaSnapshot;
use infiltrator_shared::locales::Lang;

#[test]
fn quota_status_label_has_no_fake_success_for_missing_metadata() {
    let lang = Lang("en-US");
    let snapshot = SubscriptionQuotaSnapshot::unsupported(1, 1, "provider unavailable");
    let presentation = project_quota(&snapshot, &Lang(lang.0));
    assert_eq!(
        presentation.status,
        "Quota unavailable: provider unavailable"
    );
    assert_eq!(status_kind(presentation.grade), BadgeKind::Neutral);
}

#[test]
fn native_quota_progress_layout_matches_real_zero_quarter_and_complete_fraction() {
    use iced::Size;
    use iced::advanced::layout::Limits;
    use iced::advanced::widget::Tree;
    for (fraction, expected) in [(0.0, 0.0), (0.25, 60.0), (1.0, 240.0)] {
        let mut element = usage_bar::<()>(fraction);
        let mut tree = Tree::new(element.as_widget());
        let node = element.as_widget_mut().layout(
            &mut tree,
            &(),
            &Limits::new(Size::ZERO, Size::new(240.0, 8.0)),
        );
        assert_eq!(node.bounds().width, 240.0);
        assert_eq!(node.bounds().height, 8.0);
        let row = &node.children()[0];
        let fill = &row.children()[0];
        assert!(
            (fill.bounds().width - expected).abs() < 0.1,
            "reported {fraction} used wrong native width: {:?}",
            fill.bounds()
        );
    }
}

#[test]
fn quota_progress_bar_renders_warning_and_danger_grades() {
    use iced::Size;
    use iced::advanced::layout::Limits;
    use iced::advanced::widget::Tree;
    for (fraction, grade, expected_width) in [
        (0.85, QuotaGrade::Warning, 204.0),
        (0.95, QuotaGrade::Danger, 228.0),
    ] {
        let mut element = usage_bar_with_grade::<()>(fraction, grade);
        let mut tree = Tree::new(element.as_widget());
        let node = element.as_widget_mut().layout(
            &mut tree,
            &(),
            &Limits::new(Size::ZERO, Size::new(240.0, 8.0)),
        );
        assert_eq!(node.bounds().width, 240.0);
        assert_eq!(node.bounds().height, 8.0);
        let row = &node.children()[0];
        let fill = &row.children()[0];
        assert!(
            (fill.bounds().width - expected_width).abs() < 0.1,
            "grade {grade:?} reported width {:?}",
            fill.bounds()
        );
    }
}
