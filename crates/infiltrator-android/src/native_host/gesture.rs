//! EFFECTS-047（#191）：Android 宿主的触摸手势消歧。
//!
//! Kotlin 宿主把 Android 的原始指针事件归一化成 typed 采样推给本模块，
//! 本模块是这条规则唯一的真源：位移容差、方向判定与超时判定都在这里。
//! Compose 表面只消费 typed 判决，不自己推导「这次到底是点击还是滑动」。
//!
//! 判定是纯逻辑：一个采样序列进去，一个 typed 结果出来，因此可以在
//! nextest 里穷举验证，不需要真机。

/// 点击容差（物理像素）。位移超过这个门限就判定为滑动。
pub const TAP_TOLERANCE_PX: f32 = 10.0;

/// 长按超时（毫秒）。
pub const LONG_PRESS_MILLIS: u64 = 500;

/// 宿主上报的原始触摸阶段。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    /// 指针按下。
    Started,
    /// 指针移动。
    Moved,
    /// 指针抬起。
    Ended,
    /// 手势被平台取消。
    Canceled,
}

impl TouchPhase {
    /// 解码宿主上报的触摸阶段线码。
    pub const fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Started),
            1 => Some(Self::Moved),
            2 => Some(Self::Ended),
            3 => Some(Self::Canceled),
            _ => None,
        }
    }

    /// 触摸阶段的线码（Kotlin 常量镜像这些值）。
    pub const fn as_code(self) -> i32 {
        match self {
            Self::Started => 0,
            Self::Moved => 1,
            Self::Ended => 2,
            Self::Canceled => 3,
        }
    }
}

/// 滑动的主方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollDirection {
    /// 纵向滑动（列表的常规方向）。
    Vertical,
    /// 横向滑动。
    Horizontal,
}

/// 手势仲裁的 typed 结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureOutcome {
    /// 还没有结论：手势仍在进行。
    Undecided,
    /// 点击：宿主可以派发子项动作。
    Tap,
    /// 长按：指针在容差内停留超过长按超时。
    LongPress,
    /// 滑动：宿主拥有这次滑动，不得派发子项动作。
    Scroll { direction: ScrollDirection },
    /// 多点触控：第二个指针加入，这次手势不可能是点击。
    MultiTouch,
    /// 手势被平台取消。
    Canceled,
}

impl GestureOutcome {
    /// 仲裁结果的线码（Kotlin 常量镜像这些值）。
    pub const fn as_code(self) -> i32 {
        match self {
            Self::Undecided => 0,
            Self::Tap => 1,
            Self::LongPress => 2,
            Self::Scroll {
                direction: ScrollDirection::Vertical,
            } => 3,
            Self::Scroll {
                direction: ScrollDirection::Horizontal,
            } => 4,
            Self::MultiTouch => 5,
            Self::Canceled => 6,
        }
    }

    /// 解码仲裁结果的线码。
    pub const fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Undecided),
            1 => Some(Self::Tap),
            2 => Some(Self::LongPress),
            3 => Some(Self::Scroll {
                direction: ScrollDirection::Vertical,
            }),
            4 => Some(Self::Scroll {
                direction: ScrollDirection::Horizontal,
            }),
            5 => Some(Self::MultiTouch),
            6 => Some(Self::Canceled),
            _ => None,
        }
    }

    /// 这次判决是否阻断子项动作。滑动、长按、多点触控与取消都阻断。
    pub const fn blocks_row_action(self) -> bool {
        match self {
            Self::Undecided | Self::Tap => false,
            Self::LongPress | Self::Scroll { .. } | Self::MultiTouch | Self::Canceled => true,
        }
    }
}

/// 宿主上报的物理像素坐标。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TouchPoint {
    pub x: f32,
    pub y: f32,
}

impl TouchPoint {
    /// 归一化后的坐标：非有限值折叠为 0，坏采样不会污染判决。
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x: finite(x),
            y: finite(y),
        }
    }

    /// 到另一个点的欧氏距离。
    pub fn distance_to(self, other: Self) -> f32 {
        let delta_x = self.x - other.x;
        let delta_y = self.y - other.y;
        (delta_x * delta_x + delta_y * delta_y).sqrt()
    }
}

fn finite(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

/// 一次归一化的指针采样。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GestureSample {
    /// 指针编号；只有主指针的采样参与判决。
    pub pointer_id: i64,
    pub phase: TouchPhase,
    pub x: f32,
    pub y: f32,
    /// 相对按下时刻的毫秒数。
    pub time_millis: u64,
}

impl GestureSample {
    pub const fn new(pointer_id: i64, phase: TouchPhase, x: f32, y: f32, time_millis: u64) -> Self {
        Self {
            pointer_id,
            phase,
            x,
            y,
            time_millis,
        }
    }
}

/// 仲裁策略：位移容差与长按超时。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GesturePolicy {
    /// 点击容差（物理像素）。
    pub tap_tolerance_px: f32,
    /// 长按超时（毫秒）。
    pub long_press_millis: u64,
}

impl GesturePolicy {
    /// 产品默认策略：10px 容差、500ms 长按。
    pub const DEFAULT: Self = Self {
        tap_tolerance_px: TAP_TOLERANCE_PX,
        long_press_millis: LONG_PRESS_MILLIS,
    };
}

impl Default for GesturePolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// 被跟踪手势的 typed 阶段。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GesturePhase {
    /// 没有按下的指针。
    Idle,
    /// 指针已按下，尚未越过点击容差。
    Pressing {
        origin: TouchPoint,
        started_millis: u64,
    },
    /// 指针越过了点击容差：这次手势是滑动。
    Scrolling {
        origin: TouchPoint,
        direction: ScrollDirection,
        displacement_px: f32,
    },
    /// 第二个指针加入：这次手势不可能是点击。
    MultiTouch,
}

/// 手势仲裁器：把采样序列收敛成一个 typed 判决。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GestureArbitrator {
    policy: GesturePolicy,
    phase: GesturePhase,
    pointer_id: i64,
}

impl GestureArbitrator {
    pub const fn new(policy: GesturePolicy) -> Self {
        Self {
            policy,
            phase: GesturePhase::Idle,
            pointer_id: 0,
        }
    }

    /// 当前跟踪的 typed 阶段。
    pub const fn phase(&self) -> GesturePhase {
        self.phase
    }

    /// 应用一个采样，返回当前的 typed 判决。
    pub fn apply(&mut self, sample: GestureSample) -> GestureOutcome {
        match sample.phase {
            TouchPhase::Started => self.on_started(sample),
            TouchPhase::Moved => self.on_moved(sample),
            TouchPhase::Ended => self.on_ended(sample),
            TouchPhase::Canceled => self.on_canceled(),
        }
    }

    fn on_started(&mut self, sample: GestureSample) -> GestureOutcome {
        self.pointer_id = sample.pointer_id;
        self.phase = match self.phase {
            GesturePhase::Idle => GesturePhase::Pressing {
                origin: TouchPoint::new(sample.x, sample.y),
                started_millis: sample.time_millis,
            },
            GesturePhase::Pressing { .. } | GesturePhase::Scrolling { .. } => {
                GesturePhase::MultiTouch
            }
            GesturePhase::MultiTouch => GesturePhase::MultiTouch,
        };
        GestureOutcome::Undecided
    }

    fn on_moved(&mut self, sample: GestureSample) -> GestureOutcome {
        match self.phase {
            GesturePhase::Idle => GestureOutcome::Undecided,
            GesturePhase::MultiTouch => GestureOutcome::MultiTouch,
            GesturePhase::Pressing { origin, .. } => {
                if sample.pointer_id != self.pointer_id {
                    return GestureOutcome::Undecided;
                }
                let point = TouchPoint::new(sample.x, sample.y);
                let displacement = origin.distance_to(point);
                if displacement > self.policy.tap_tolerance_px {
                    let direction = dominant_direction(sample.x - origin.x, sample.y - origin.y);
                    self.phase = GesturePhase::Scrolling {
                        origin,
                        direction,
                        displacement_px: displacement,
                    };
                    GestureOutcome::Scroll { direction }
                } else {
                    GestureOutcome::Undecided
                }
            }
            GesturePhase::Scrolling {
                origin, direction, ..
            } => {
                if sample.pointer_id != self.pointer_id {
                    return GestureOutcome::Scroll { direction };
                }
                let displacement = origin.distance_to(TouchPoint::new(sample.x, sample.y));
                self.phase = GesturePhase::Scrolling {
                    origin,
                    direction,
                    displacement_px: displacement,
                };
                GestureOutcome::Scroll { direction }
            }
        }
    }

    fn on_ended(&mut self, sample: GestureSample) -> GestureOutcome {
        let point = TouchPoint::new(sample.x, sample.y);
        let outcome = match self.phase {
            GesturePhase::Idle => return GestureOutcome::Undecided,
            GesturePhase::MultiTouch => GestureOutcome::MultiTouch,
            GesturePhase::Pressing {
                origin,
                started_millis,
            } => {
                if sample.pointer_id != self.pointer_id {
                    return GestureOutcome::Undecided;
                }
                let displacement = origin.distance_to(point);
                if displacement > self.policy.tap_tolerance_px {
                    GestureOutcome::Scroll {
                        direction: dominant_direction(sample.x - origin.x, sample.y - origin.y),
                    }
                } else if sample.time_millis.saturating_sub(started_millis)
                    >= self.policy.long_press_millis
                {
                    GestureOutcome::LongPress
                } else {
                    GestureOutcome::Tap
                }
            }
            GesturePhase::Scrolling { direction, .. } => GestureOutcome::Scroll { direction },
        };
        self.phase = GesturePhase::Idle;
        outcome
    }

    fn on_canceled(&mut self) -> GestureOutcome {
        let outcome = match self.phase {
            GesturePhase::Idle => GestureOutcome::Undecided,
            GesturePhase::MultiTouch => GestureOutcome::MultiTouch,
            GesturePhase::Pressing { .. } | GesturePhase::Scrolling { .. } => {
                GestureOutcome::Canceled
            }
        };
        self.phase = GesturePhase::Idle;
        outcome
    }
}

impl Default for GestureArbitrator {
    fn default() -> Self {
        Self::new(GesturePolicy::DEFAULT)
    }
}

/// 判定滑动的主方向：位移较大的那条轴；平手时按列表的纵向处理。
fn dominant_direction(delta_x: f32, delta_y: f32) -> ScrollDirection {
    if delta_x.abs() > delta_y.abs() {
        ScrollDirection::Horizontal
    } else {
        ScrollDirection::Vertical
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(phase: TouchPhase, x: f32, y: f32, time_millis: u64) -> GestureSample {
        GestureSample::new(1, phase, x, y, time_millis)
    }

    #[test]
    fn wire_codes_round_trip_and_unknown_is_rejected() {
        for phase in [
            TouchPhase::Started,
            TouchPhase::Moved,
            TouchPhase::Ended,
            TouchPhase::Canceled,
        ] {
            assert_eq!(TouchPhase::from_code(phase.as_code()), Some(phase));
        }
        assert_eq!(TouchPhase::from_code(-1), None);
        assert_eq!(TouchPhase::from_code(4), None);

        for outcome in [
            GestureOutcome::Undecided,
            GestureOutcome::Tap,
            GestureOutcome::LongPress,
            GestureOutcome::Scroll {
                direction: ScrollDirection::Vertical,
            },
            GestureOutcome::Scroll {
                direction: ScrollDirection::Horizontal,
            },
            GestureOutcome::MultiTouch,
            GestureOutcome::Canceled,
        ] {
            assert_eq!(GestureOutcome::from_code(outcome.as_code()), Some(outcome));
        }
        assert_eq!(GestureOutcome::from_code(7), None);
    }

    #[test]
    fn a_pointer_inside_the_tolerance_is_a_tap() {
        let mut arbitrator = GestureArbitrator::default();

        assert_eq!(
            arbitrator.apply(sample(TouchPhase::Started, 100.0, 200.0, 0)),
            GestureOutcome::Undecided
        );
        // 9px 的抖动仍在 10px 容差内。
        assert_eq!(
            arbitrator.apply(sample(TouchPhase::Moved, 106.0, 203.0, 40)),
            GestureOutcome::Undecided
        );
        assert_eq!(
            arbitrator.apply(sample(TouchPhase::Ended, 106.0, 203.0, 120)),
            GestureOutcome::Tap
        );
        assert_eq!(arbitrator.phase(), GesturePhase::Idle);
    }

    #[test]
    fn a_pointer_beyond_the_tolerance_is_a_scroll_and_never_a_tap() {
        let mut arbitrator = GestureArbitrator::default();

        arbitrator.apply(sample(TouchPhase::Started, 100.0, 200.0, 0));
        // 11px 已经越过 10px 容差。
        assert_eq!(
            arbitrator.apply(sample(TouchPhase::Moved, 111.0, 200.0, 30)),
            GestureOutcome::Scroll {
                direction: ScrollDirection::Horizontal,
            }
        );
        assert_eq!(
            arbitrator.apply(sample(TouchPhase::Ended, 111.0, 200.0, 90)),
            GestureOutcome::Scroll {
                direction: ScrollDirection::Horizontal,
            }
        );
    }

    #[test]
    fn a_release_beyond_the_tolerance_is_a_scroll_even_without_a_move() {
        let mut arbitrator = GestureArbitrator::default();

        arbitrator.apply(sample(TouchPhase::Started, 100.0, 200.0, 0));
        assert_eq!(
            arbitrator.apply(sample(TouchPhase::Ended, 100.0, 260.0, 80)),
            GestureOutcome::Scroll {
                direction: ScrollDirection::Vertical,
            }
        );
    }

    #[test]
    fn the_dominant_axis_owns_the_scroll_direction() {
        let mut vertical = GestureArbitrator::default();
        vertical.apply(sample(TouchPhase::Started, 100.0, 200.0, 0));
        assert_eq!(
            vertical.apply(sample(TouchPhase::Ended, 104.0, 260.0, 60)),
            GestureOutcome::Scroll {
                direction: ScrollDirection::Vertical,
            }
        );

        let mut horizontal = GestureArbitrator::default();
        horizontal.apply(sample(TouchPhase::Started, 100.0, 200.0, 0));
        assert_eq!(
            horizontal.apply(sample(TouchPhase::Ended, 260.0, 204.0, 60)),
            GestureOutcome::Scroll {
                direction: ScrollDirection::Horizontal,
            }
        );
    }

    #[test]
    fn a_press_longer_than_the_timeout_is_a_long_press() {
        let mut arbitrator = GestureArbitrator::default();

        arbitrator.apply(sample(TouchPhase::Started, 100.0, 200.0, 0));
        assert_eq!(
            arbitrator.apply(sample(TouchPhase::Ended, 101.0, 201.0, 600)),
            GestureOutcome::LongPress
        );
        assert!(GestureOutcome::LongPress.blocks_row_action());
        assert!(!GestureOutcome::Tap.blocks_row_action());
    }

    #[test]
    fn a_canceled_gesture_never_becomes_a_tap() {
        let mut arbitrator = GestureArbitrator::default();

        arbitrator.apply(sample(TouchPhase::Started, 100.0, 200.0, 0));
        assert_eq!(
            arbitrator.apply(sample(TouchPhase::Canceled, 100.0, 200.0, 30)),
            GestureOutcome::Canceled
        );
        assert_eq!(arbitrator.phase(), GesturePhase::Idle);
        assert!(GestureOutcome::Canceled.blocks_row_action());
    }

    #[test]
    fn a_second_pointer_turns_the_gesture_into_multi_touch() {
        let mut arbitrator = GestureArbitrator::default();

        arbitrator.apply(sample(TouchPhase::Started, 100.0, 200.0, 0));
        let second = GestureSample::new(2, TouchPhase::Started, 300.0, 200.0, 20);
        assert_eq!(arbitrator.apply(second), GestureOutcome::Undecided);
        assert_eq!(arbitrator.phase(), GesturePhase::MultiTouch);
        assert_eq!(
            arbitrator.apply(sample(TouchPhase::Ended, 101.0, 200.0, 90)),
            GestureOutcome::MultiTouch
        );
        assert!(GestureOutcome::MultiTouch.blocks_row_action());
    }

    #[test]
    fn a_foreign_pointer_cannot_end_the_tracked_gesture() {
        let mut arbitrator = GestureArbitrator::default();

        arbitrator.apply(sample(TouchPhase::Started, 100.0, 200.0, 0));
        let foreign = GestureSample::new(7, TouchPhase::Ended, 100.0, 200.0, 90);
        assert_eq!(arbitrator.apply(foreign), GestureOutcome::Undecided);
        // 别人的指针不能结束这次手势，主指针仍然在跟踪中。
        assert_eq!(
            arbitrator.phase(),
            GesturePhase::Pressing {
                origin: TouchPoint::new(100.0, 200.0),
                started_millis: 0,
            }
        );
    }

    #[test]
    fn a_non_finite_sample_collapses_instead_of_poisoning_the_verdict() {
        let mut arbitrator = GestureArbitrator::default();

        arbitrator.apply(sample(TouchPhase::Started, f32::NAN, f32::INFINITY, 0));
        assert_eq!(
            arbitrator.apply(sample(TouchPhase::Ended, f32::NAN, f32::NAN, 50)),
            GestureOutcome::Tap
        );
    }
}
