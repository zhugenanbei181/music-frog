//! `performance_watchdog` 的行为用例：降档的持续性判定与冷却、最低档的
//! 保持、纹理预算的 typed 拒绝与换档重挂。

use super::*;

#[test]
fn a_single_spike_never_demotes() {
    let mut governor = FrameTimeGovernor::new(VisualTier::Enhanced);
    for _ in 0..29 {
        assert_eq!(governor.observe(4.0), GovernorDecision::Hold);
    }
    // 第 30 帧之前只有零星尖刺：一次 20ms 不够降档。
    assert_eq!(governor.observe(4.0), GovernorDecision::Hold);
    assert_eq!(governor.observe(20.0), GovernorDecision::Hold);
    assert_eq!(governor.observe(4.0), GovernorDecision::Hold);
}

#[test]
fn a_sustained_streak_demotes_exactly_one_tier() {
    let mut governor = FrameTimeGovernor::new(VisualTier::Enhanced);
    for _ in 0..(DEMOTION_STREAK_FRAMES - 1) {
        assert_eq!(governor.observe(17.4), GovernorDecision::Hold);
    }
    assert_eq!(
        governor.observe(17.4),
        GovernorDecision::DemoteTo(VisualTier::Standard)
    );
    assert_eq!(governor.tier(), VisualTier::Standard);
    // 降档后立刻进入冷却：继续超标也只保持。
    for _ in 0..DEMOTION_COOLDOWN_FRAMES {
        assert_eq!(governor.observe(30.0), GovernorDecision::Hold);
    }
    assert_eq!(governor.tier(), VisualTier::Standard);
}

#[test]
fn the_lowest_tier_holds_no_matter_what() {
    let mut governor = FrameTimeGovernor::new(VisualTier::Simplified);
    for _ in 0..(DEMOTION_STREAK_FRAMES * 3) {
        assert_eq!(governor.observe(50.0), GovernorDecision::Hold);
    }
    assert_eq!(governor.tier(), VisualTier::Simplified);
}

#[test]
fn a_healthy_streak_resets_the_counter() {
    let mut governor = FrameTimeGovernor::new(VisualTier::Enhanced);
    for _ in 0..(DEMOTION_STREAK_FRAMES - 1) {
        governor.observe(20.0);
    }
    // 一帧健康帧清零计数，随后需要重新攒满 30 帧。
    governor.observe(5.0);
    for frame in 0..(DEMOTION_STREAK_FRAMES - 1) {
        assert_eq!(governor.observe(20.0), GovernorDecision::Hold, "frame {frame}");
    }
    assert_eq!(
        governor.observe(20.0),
        GovernorDecision::DemoteTo(VisualTier::Standard)
    );
}

#[test]
fn the_texture_ledger_allows_within_and_denies_beyond_the_tier_budget() {
    let mut ledger = TextureBudgetLedger::new(VisualTier::Simplified);
    assert_eq!(ledger.budget_bytes(), 64 * 1024 * 1024);
    assert_eq!(ledger.request(32 * 1024 * 1024), BudgetDecision::Allow);
    assert_eq!(ledger.resident_bytes(), 32 * 1024 * 1024);
    // 第二笔同样大小超预算：typed 拒绝且不记账。
    assert_eq!(
        ledger.request(32 * 1024 * 1024 + 1),
        BudgetDecision::Deny {
            resident_bytes: 32 * 1024 * 1024,
            budget_bytes: 64 * 1024 * 1024,
        }
    );
    assert_eq!(ledger.resident_bytes(), 32 * 1024 * 1024);
    // 释放后同额申请重新放行。
    ledger.release(32 * 1024 * 1024);
    assert_eq!(ledger.request(64 * 1024 * 1024), BudgetDecision::Allow);
}

#[test]
fn the_ledger_rebudgets_when_the_tier_changes() {
    let mut ledger = TextureBudgetLedger::new(VisualTier::Enhanced);
    assert_eq!(ledger.request(200 * 1024 * 1024), BudgetDecision::Allow);
    // 降到 Simplified：常驻量保留，预算按新档位取，追加被拒。
    ledger.retier(VisualTier::Simplified);
    assert_eq!(ledger.budget_bytes(), 64 * 1024 * 1024);
    assert_eq!(
        ledger.request(1),
        BudgetDecision::Deny {
            resident_bytes: 200 * 1024 * 1024,
            budget_bytes: 64 * 1024 * 1024,
        }
    );
    // 释放到预算内后恢复放行。
    ledger.release(150 * 1024 * 1024);
    assert_eq!(ledger.request(1), BudgetDecision::Allow);
}
