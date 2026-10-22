//! EFFECTS-047（#190）：Android Surface 的销毁与重建。
//!
//! Kotlin 宿主是平台 Surface 的唯一观测者：它把 `SurfaceHolder` 的回调
//! 归一化成 typed 事实推给本模块，本模块是 Surface 生命周期唯一的真源。
//! 代（generation）栅栏保证已经退役的 Activity 不能覆盖新一代的状态，
//! 共享外壳因此能区分「Surface 可用」「Surface 已被销毁」与「新一代正在
//! 重建 Surface」，恢复时不会往一个已经死掉的 Surface 上渲染。

/// 宿主对自己渲染 Surface 的 typed 报告。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostSurfaceState {
    /// Surface 已创建，但平台还没有给出尺寸。
    Created,
    /// Surface 可用，携带它的像素尺寸。
    Live { width_px: u32, height_px: u32 },
    /// Surface 已被平台销毁。
    Destroyed,
}

impl HostSurfaceState {
    /// 解码宿主上报的 Surface 事实。`Live` 携带平台给出的像素尺寸。
    pub fn from_code(code: i32, width_px: i32, height_px: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Created),
            1 => Some(Self::Live {
                width_px: u32::try_from(width_px).unwrap_or(0),
                height_px: u32::try_from(height_px).unwrap_or(0),
            }),
            2 => Some(Self::Destroyed),
            _ => None,
        }
    }

    /// Surface 事实的线码（Kotlin 常量镜像这些值）。
    pub const fn as_code(self) -> i32 {
        match self {
            Self::Created => 0,
            Self::Live { .. } => 1,
            Self::Destroyed => 2,
        }
    }
}

/// 宿主推送的一条 Surface 事实。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceFact {
    /// 单调递增的 Activity 实例代号；重建后的实例代号更大。
    pub generation: u64,
    pub state: HostSurfaceState,
}

impl SurfaceFact {
    pub const fn new(generation: u64, state: HostSurfaceState) -> Self {
        Self { generation, state }
    }
}

/// 共享外壳消费的 typed Surface 生命周期。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceLifecycleState {
    /// 还没有收到任何 Surface 事实。
    Unobserved,
    /// Surface 可用，外壳可以往里渲染。
    Live {
        generation: u64,
        width_px: u32,
        height_px: u32,
    },
    /// Surface 已被销毁：不得再往里渲染，必须等重建。
    TornDown { generation: u64 },
    /// 新的 Activity 代正在重建 Surface。
    Rebuilding { generation: u64 },
}

impl SurfaceLifecycleState {
    /// 外壳此刻是否可以往 Surface 里渲染。
    pub const fn can_render(self) -> bool {
        match self {
            Self::Unobserved => false,
            Self::Live { .. } => true,
            Self::TornDown { .. } | Self::Rebuilding { .. } => false,
        }
    }

    /// Surface 是否已经被销毁：宿主此刻没有可以承载触摸的表面。
    pub const fn is_torn_down(self) -> bool {
        matches!(self, Self::TornDown { .. })
    }

    /// 把一条事实投影成 typed 生命周期。
    fn project(generation: u64, state: HostSurfaceState) -> Self {
        match state {
            HostSurfaceState::Created => Self::Rebuilding { generation },
            HostSurfaceState::Live {
                width_px,
                height_px,
            } => Self::Live {
                generation,
                width_px,
                height_px,
            },
            HostSurfaceState::Destroyed => Self::TornDown { generation },
        }
    }
}

/// 应用一条 Surface 事实的 typed 结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceUpdate {
    /// 事实被接受。
    Applied,
    /// 新的 Activity 代替换了上一代：Surface 需要重建。
    Recreated,
    /// 事实来自已经退役的代，被忽略。
    Stale,
}

/// 跟踪最新的 Surface 事实，并用代号栅栏围住 Activity 重建。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceLifecycleTracker {
    current: SurfaceLifecycleState,
}

impl SurfaceLifecycleTracker {
    pub const fn new() -> Self {
        Self {
            current: SurfaceLifecycleState::Unobserved,
        }
    }

    /// 当前的 typed Surface 生命周期。
    pub const fn current(&self) -> SurfaceLifecycleState {
        self.current
    }

    /// 应用一条事实。退役代的迟到事实被忽略，新的一代替换旧的一代。
    pub fn apply(&mut self, fact: SurfaceFact) -> SurfaceUpdate {
        match self.current {
            SurfaceLifecycleState::Unobserved => {
                self.current = SurfaceLifecycleState::project(fact.generation, fact.state);
                SurfaceUpdate::Applied
            }
            SurfaceLifecycleState::Live { generation, .. }
            | SurfaceLifecycleState::TornDown { generation }
            | SurfaceLifecycleState::Rebuilding { generation } => {
                if fact.generation < generation {
                    return SurfaceUpdate::Stale;
                }
                if fact.generation > generation {
                    self.current = SurfaceLifecycleState::project(fact.generation, fact.state);
                    return SurfaceUpdate::Recreated;
                }
                self.current = SurfaceLifecycleState::project(fact.generation, fact.state);
                SurfaceUpdate::Applied
            }
        }
    }
}

impl Default for SurfaceLifecycleTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live(width_px: u32, height_px: u32) -> HostSurfaceState {
        HostSurfaceState::Live {
            width_px,
            height_px,
        }
    }

    #[test]
    fn wire_codes_round_trip_and_unknown_is_rejected() {
        assert_eq!(
            HostSurfaceState::from_code(0, 0, 0),
            Some(HostSurfaceState::Created)
        );
        assert_eq!(
            HostSurfaceState::from_code(1, 1080, 1920),
            Some(live(1080, 1920))
        );
        assert_eq!(
            HostSurfaceState::from_code(2, 0, 0),
            Some(HostSurfaceState::Destroyed)
        );
        assert_eq!(HostSurfaceState::from_code(3, 0, 0), None);
        assert_eq!(HostSurfaceState::from_code(-1, 0, 0), None);
        assert_eq!(HostSurfaceState::Created.as_code(), 0);
        assert_eq!(live(1, 2).as_code(), 1);
        assert_eq!(HostSurfaceState::Destroyed.as_code(), 2);
    }

    #[test]
    fn a_destroyed_surface_is_never_renderable_until_it_is_rebuilt() {
        let mut tracker = SurfaceLifecycleTracker::new();

        assert_eq!(
            tracker.apply(SurfaceFact::new(4, HostSurfaceState::Created)),
            SurfaceUpdate::Applied
        );
        assert_eq!(
            tracker.current(),
            SurfaceLifecycleState::Rebuilding { generation: 4 }
        );
        assert!(!tracker.current().can_render());

        assert_eq!(
            tracker.apply(SurfaceFact::new(4, live(1080, 1920))),
            SurfaceUpdate::Applied
        );
        assert_eq!(
            tracker.current(),
            SurfaceLifecycleState::Live {
                generation: 4,
                width_px: 1080,
                height_px: 1920,
            }
        );
        assert!(tracker.current().can_render());

        assert_eq!(
            tracker.apply(SurfaceFact::new(4, HostSurfaceState::Destroyed)),
            SurfaceUpdate::Applied
        );
        assert_eq!(
            tracker.current(),
            SurfaceLifecycleState::TornDown { generation: 4 }
        );
        assert!(!tracker.current().can_render());
    }

    #[test]
    fn the_same_activity_recreates_its_surface_after_a_teardown() {
        let mut tracker = SurfaceLifecycleTracker::new();

        tracker.apply(SurfaceFact::new(9, live(1080, 1920)));
        tracker.apply(SurfaceFact::new(9, HostSurfaceState::Destroyed));
        assert_eq!(
            tracker.apply(SurfaceFact::new(9, HostSurfaceState::Created)),
            SurfaceUpdate::Applied
        );
        assert_eq!(
            tracker.current(),
            SurfaceLifecycleState::Rebuilding { generation: 9 }
        );
        assert_eq!(
            tracker.apply(SurfaceFact::new(9, live(1920, 1080))),
            SurfaceUpdate::Applied
        );
        assert_eq!(
            tracker.current(),
            SurfaceLifecycleState::Live {
                generation: 9,
                width_px: 1920,
                height_px: 1080,
            }
        );
    }

    #[test]
    fn a_retired_activity_cannot_overwrite_the_recreated_one() {
        let mut tracker = SurfaceLifecycleTracker::new();

        tracker.apply(SurfaceFact::new(5, live(1080, 1920)));
        assert_eq!(
            tracker.apply(SurfaceFact::new(4, HostSurfaceState::Destroyed)),
            SurfaceUpdate::Stale
        );
        assert_eq!(
            tracker.current(),
            SurfaceLifecycleState::Live {
                generation: 5,
                width_px: 1080,
                height_px: 1920,
            }
        );

        assert_eq!(
            tracker.apply(SurfaceFact::new(6, HostSurfaceState::Created)),
            SurfaceUpdate::Recreated
        );
        assert_eq!(
            tracker.current(),
            SurfaceLifecycleState::Rebuilding { generation: 6 }
        );
    }
}
