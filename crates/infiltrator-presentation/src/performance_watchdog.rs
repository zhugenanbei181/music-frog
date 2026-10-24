//! 动态画质自适应与纹理预算治理（EFFECTS-048 #193~#195）。
//!
//! 纯 Core：三个 typed 判定器，不做任何 I/O——宿主把测得的帧时长与纹理
//! 字节数喂进来，拿走 typed 决策。
//!
//! * [`FrameTimeGovernor`]（#194）：帧时长持续超预算（>16.6ms）就自动降一
//!   档画质并进入冷却；冷却期内的继续超标保持档位不再连降，恢复只能由
//!   使用者在设置里显式提档——降档自动、升档手动，杜绝振荡。
//! * [`TextureBudgetLedger`]（#195）：按档位预算（契约 `VisualTier` 的
//!   `texture_budget_bytes`）记账，超预算的分配被 typed 拒绝而不是静默
//!   溢出。
//! * 后台暂停（#193）不在本模块：后台 1fps/休眠由 bevy-ui 的
//!   `cadence`（契约 `RenderCadence` 的镜像）与 winit 低功耗档承担。

use infiltrator_contract::visual_tier::VisualTier;

/// 交互帧预算：60fps 的帧间隔（毫秒）。
pub const INTERACTIVE_FRAME_BUDGET_MS: f32 = 16.6;

/// 降档判定需要的连续超标帧数：一次尖刺（GC、窗口抖动）不构成降档。
pub const DEMOTION_STREAK_FRAMES: u32 = 30;

/// 降档后的冷却帧数：冷却期内不重复降档。
pub const DEMOTION_COOLDOWN_FRAMES: u32 = 120;

/// 下一低档；已在 `Simplified` 时为 `None`。降档序是本模块的裁决，
/// 契约只定义档位事实。
fn lower_tier(tier: VisualTier) -> Option<VisualTier> {
    match tier {
        VisualTier::Enhanced => Some(VisualTier::Standard),
        VisualTier::Standard => Some(VisualTier::Simplified),
        VisualTier::Simplified => None,
    }
}

/// 帧时长治理器的决策。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GovernorDecision {
    /// 保持当前档位。
    Hold,
    /// 降一档画质（携带降到的档位）。
    DemoteTo(VisualTier),
}

/// 帧时长治理器（#194）：>16.6ms 持续 30 帧自动降一档，降档后进入
/// 120 帧冷却。
///
/// 阶段是 typed 的：`Tracking`（盯超标计数）与 `Cooling`（降档后的冷却
/// 倒计时）两态互斥，不是一个可组合的 bool 对。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameTimeGovernor {
    tier: VisualTier,
    phase: GovernorPhase,
}

/// 治理器的内部阶段。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GovernorPhase {
    /// 正在跟踪超标计数。
    Tracking { over_budget_streak: u32 },
    /// 降档后的冷却倒计时。
    Cooling { remaining: u32 },
}

impl FrameTimeGovernor {
    /// 以某个画质档位启动治理。
    pub fn new(tier: VisualTier) -> Self {
        Self {
            tier,
            phase: GovernorPhase::Tracking {
                over_budget_streak: 0,
            },
        }
    }

    /// 当前守护的档位。
    pub fn tier(self) -> VisualTier {
        self.tier
    }

    /// 观察一帧的时长并给出决策。降到 `Simplified` 后再超标也保持不动
    /// （最低档没有更低的去处），由使用者提档或改善负载。
    pub fn observe(&mut self, frame_ms: f32) -> GovernorDecision {
        match &mut self.phase {
            GovernorPhase::Tracking { over_budget_streak } => {
                if frame_ms > INTERACTIVE_FRAME_BUDGET_MS {
                    *over_budget_streak += 1;
                } else {
                    *over_budget_streak = 0;
                }
                if *over_budget_streak >= DEMOTION_STREAK_FRAMES {
                    match lower_tier(self.tier) {
                        Some(lower) => {
                            self.tier = lower;
                            self.phase = GovernorPhase::Cooling {
                                remaining: DEMOTION_COOLDOWN_FRAMES,
                            };
                            GovernorDecision::DemoteTo(lower)
                        }
                        // 已在最低档：重置计数继续观察，不再重复降档。
                        None => {
                            self.phase = GovernorPhase::Tracking {
                                over_budget_streak: 0,
                            };
                            GovernorDecision::Hold
                        }
                    }
                } else {
                    GovernorDecision::Hold
                }
            }
            GovernorPhase::Cooling { remaining } => {
                *remaining = remaining.saturating_sub(1);
                if *remaining == 0 {
                    self.phase = GovernorPhase::Tracking {
                        over_budget_streak: 0,
                    };
                }
                GovernorDecision::Hold
            }
        }
    }
}

/// 纹理预算台账的决策（#195）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BudgetDecision {
    /// 预算内，放行。
    Allow,
    /// 超出档位预算，拒绝（携带当前占用与预算字节）。
    Deny {
        resident_bytes: u64,
        budget_bytes: u64,
    },
}

/// 纹理预算台账（#195）：按契约档位预算记账。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextureBudgetLedger {
    tier: VisualTier,
    resident_bytes: u64,
}

impl TextureBudgetLedger {
    /// 以某个档位的预算启动台账。
    pub fn new(tier: VisualTier) -> Self {
        Self {
            tier,
            resident_bytes: 0,
        }
    }

    /// 当前常驻字节数。
    pub fn resident_bytes(self) -> u64 {
        self.resident_bytes
    }

    /// 档位预算字节（契约 `VisualTier::texture_budget_bytes`）。
    pub fn budget_bytes(self) -> u64 {
        self.tier.texture_budget_bytes()
    }

    /// 申请驻留 `bytes`：预算内放行并记账，超出则 typed 拒绝（不记账）。
    pub fn request(&mut self, bytes: u64) -> BudgetDecision {
        let budget = self.budget_bytes();
        match self.resident_bytes.checked_add(bytes) {
            Some(total) if total <= budget => {
                self.resident_bytes = total;
                BudgetDecision::Allow
            }
            _ => BudgetDecision::Deny {
                resident_bytes: self.resident_bytes,
                budget_bytes: budget,
            },
        }
    }

    /// 释放驻留（下策不会超过已记账量）。
    pub fn release(&mut self, bytes: u64) {
        self.resident_bytes = self.resident_bytes.saturating_sub(bytes);
    }

    /// 档位变化时重挂预算：常驻量保留，预算按新档位取。
    pub fn retier(&mut self, tier: VisualTier) {
        self.tier = tier;
    }
}

#[cfg(test)]
#[path = "performance_watchdog_tests.rs"]
mod performance_watchdog_tests;
