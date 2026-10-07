//! Behavior cases for subscription.
//! test-intent: behavior

use super::*;
use bevy::ui;

/// The Overview page mounts the subscription quota card (BEVY-GAP-020)
/// with title, header, subtitle/stats and visual progress bar.
#[test]
fn test_subscription_quota_card_mounts_with_progress_bar() {
    let mut app = mounted_default();
    let world = app.world_mut();

    let mut query = world.query::<(Entity, &SubscriptionQuotaCard)>();
    let (card_entity, _) = query
        .iter(world)
        .next()
        .expect("SubscriptionQuotaCard must be mounted in overview page");

    let all_descendants = descendants(world, card_entity);
    let texts: Vec<String> = all_descendants
        .iter()
        .filter_map(|e| world.get::<Text>(*e).map(|t| t.0.clone()))
        .collect();

    // Card title
    assert!(texts.iter().any(|t| t == "订阅配额"), "contains 订阅配额");

    // Header uses the application-owned profile and expiry facts.
    assert!(
        texts.iter().any(|t| t == "主力高速订阅"),
        "contains active profile name"
    );
    assert!(
        texts
            .iter()
            .any(|t| t.contains("2026-10-01") && t.contains("25 天")),
        "contains expiry and remaining days"
    );

    // Metrics include real used/total bytes and the shared usage percentage.
    assert!(
        texts
            .iter()
            .any(|t| t.contains("已用") && t.contains("24.9%")),
        "contains stats"
    );
    assert!(
        texts.iter().any(|t| t == "账单重置未上报"),
        "does not infer a billing reset from expiry"
    );

    // Visual progress bar: height ~8px, inner fill width 25% with palette.accent
    let mut found_bar = false;
    for e in &all_descendants {
        if let Some(node) = world.get::<ui::Node>(*e)
            && node.height == ui::Val::Px(8.0)
        {
            let bar_descendants = descendants(world, *e);
            for child in bar_descendants {
                if let Some(inner_node) = world.get::<ui::Node>(child)
                    && matches!(inner_node.width, ui::Val::Percent(value) if (value - 24.9).abs() < 0.1)
                {
                    found_bar = true;
                    break;
                }
            }
        }
    }
    assert!(
        found_bar,
        "visual progress bar with 8px height and 25% fill mounted"
    );

    // Standalone scene creation test
    let palette = UiPalette::new(&Theme::dark());
    let _scene = subscription_quota_scene(&palette);
}

#[test]
fn native_quota_unknown_zero_and_failure_keep_label_entities_and_locale_preserves_raw_facts() {
    use infiltrator_bevy_ui::pages::overview::LastOverviewProjection;
    use infiltrator_bevy_ui::pages::overview_restamp::{
        SubscriptionQuotaText, SubscriptionQuotaTextKind,
    };
    use infiltrator_bevy_widgets::localization::UiLocale;
    use infiltrator_contract::subscription_quota::{
        SubscriptionQuotaSnapshot, SubscriptionQuotaStatus,
    };
    let mut app = mounted_default();
    let labels: Vec<_> = app
        .world_mut()
        .query::<(Entity, &SubscriptionQuotaText)>()
        .iter(app.world())
        .map(|(entity, role)| (entity, role.0))
        .collect();
    assert_eq!(labels.len(), 5);
    let mut projection = app
        .world()
        .resource::<LastOverviewProjection>()
        .0
        .clone()
        .unwrap();
    projection.subscription_quota = SubscriptionQuotaSnapshot::default();
    app.world_mut()
        .trigger(OverviewProjectionUpdated(projection.clone()));
    app.update();
    for (entity, role) in &labels {
        let copy = &app.world().get::<Text>(*entity).unwrap().0;
        match role {
            SubscriptionQuotaTextKind::Status => assert_eq!(copy, "尚未观测配额"),
            SubscriptionQuotaTextKind::Metrics => {
                assert!(copy.contains("已用 —"));
                assert!(copy.contains("未观测"));
            }
            _ => {}
        }
    }
    projection.subscription_quota = SubscriptionQuotaSnapshot {
        status: SubscriptionQuotaStatus::Ready,
        profile_name: Some("用户 {usage}".into()),
        used_bytes: Some(0),
        total_bytes: Some(100),
        remaining_bytes: Some(100),
        usage_percent: Some(0.0),
        remaining_percent: Some(100.0),
        ..Default::default()
    };
    app.world_mut()
        .trigger(OverviewProjectionUpdated(projection.clone()));
    app.update();
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("en-US");
    app.update();
    for (entity, role) in &labels {
        assert_eq!(
            app.world().get::<SubscriptionQuotaText>(*entity).unwrap().0,
            *role
        );
        let copy = &app.world().get::<Text>(*entity).unwrap().0;
        match role {
            SubscriptionQuotaTextKind::Profile => assert_eq!(copy, "用户 {usage}"),
            SubscriptionQuotaTextKind::Metrics => {
                assert!(copy.contains("Used 0 B / Total 100 B · 0.0%"));
                assert!(copy.contains("Remaining 100 B (100.0%)"));
            }
            SubscriptionQuotaTextKind::Status => assert_eq!(copy, "Provider metadata observed"),
            _ => {}
        }
    }
    projection.subscription_quota = SubscriptionQuotaSnapshot::failed(1, 3, "backend {reason}");
    app.world_mut()
        .trigger(OverviewProjectionUpdated(projection.clone()));
    app.update();
    for (entity, role) in &labels {
        if matches!(role, SubscriptionQuotaTextKind::Status) {
            assert_eq!(
                app.world().get::<Text>(*entity).unwrap().0,
                "Quota read failed: backend {reason}"
            );
            assert_eq!(
                app.world().get::<TextColor>(*entity).unwrap().0,
                app.world().resource::<UiPalette>().danger
            );
        }
    }
    app.world_mut()
        .resource_mut::<UiLocale>()
        .apply_preference("zh-CN");
    app.update();
    for (entity, role) in labels {
        if matches!(role, SubscriptionQuotaTextKind::Status) {
            assert_eq!(
                app.world().get::<Text>(entity).unwrap().0,
                "配额读取失败：backend {reason}"
            );
        }
    }
    projection.subscription_quota.retained = true;
    projection.subscription_quota.used_bytes = Some(0);
    projection.subscription_quota.total_bytes = Some(100);
    projection.subscription_quota.usage_percent = Some(0.0);
    app.world_mut()
        .trigger(OverviewProjectionUpdated(projection));
    app.update();
    let mut query = app.world_mut().query::<(&SubscriptionQuotaText, &Text)>();
    for (role, text) in query.iter(app.world()) {
        if matches!(role.0, SubscriptionQuotaTextKind::Status) {
            assert_eq!(
                text.0,
                "保留上次实际配额（已失效）· 配额读取失败：backend {reason}"
            );
        }
        if matches!(role.0, SubscriptionQuotaTextKind::Metrics) {
            assert!(text.0.contains("已用 0 B"));
        }
    }
}
