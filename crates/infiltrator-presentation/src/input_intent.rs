//! 统一的键盘导航意图与输入法组合态保护协议（EFFECTS-010 #35、#36）。
//!
//! 两个表面必须对同一个按键做出同样的反应，而任何表面都不能从用户正在
//! 输入的字段手里抢走按键。前者是意图转换：一个按键变成一个 typed 意图，
//! 而对 shell 没有意义的按键什么也不是。后者是组合态保护：输入法正在组合
//! 时，会覆盖文本框的发布值必须被抑制。
//!
//! 组合的**词汇**是共享的：输入法会话属于 `infiltrator_contract::ime`，
//! Iced 与 Bevy 都已经会说这套词汇。本模块只补上表面缺的那条规则 ——
//! *此刻这份发布值能不能写进这个字段* —— 并且把它放在一个 typed 状态机
//! 里，而不是让每个表面各写一个布尔量。
//!
//! 会话与字段是两个事实：会话在提交或取消时结束，而字段在用户刚提交完
//! 文本之后仍然显示用户的文本，直到它结算为止。因此这里的阶段是**字段**
//! 的阶段，不是会话的阶段，共享词汇仍然是唯一的会话真源。

use infiltrator_contract::ime::ImeCompositionAction;

/// shell 反应的那几个按键，用与工具链无关的词汇表达。
///
/// 工具链的按键由表面翻译成这套词汇，因此 shell 对按键的反应只写一次。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShellKey {
    /// Tab 键。
    Tab,
    /// Shift+Tab 组合键。
    ShiftTab,
    /// 回车键。
    Enter,
    /// Esc 键。
    Escape,
    /// 上方向键。
    ArrowUp,
    /// 下方向键。
    ArrowDown,
    /// 左方向键。
    ArrowLeft,
    /// 右方向键。
    ArrowRight,
    /// 退格键。
    Backspace,
    /// 任意可打印字符。
    Character(char),
}

/// 一次方向意图的方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScrollDirection {
    /// 朝内容的上方。
    Up,
    /// 朝内容的下方。
    Down,
    /// 朝内容的开头。
    Left,
    /// 朝内容的末尾。
    Right,
}

/// 一次按键可能意味着的导航意图（EFFECTS-010 #35）。
///
/// 意图只有四种：移动焦点、激活、关闭，以及一个方向。表面只负责把工具链
/// 的按键映射成 [`ShellKey`]，再把意图映射到自己的模型；键的含义只在
/// 这里写一次。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyIntent {
    /// 把焦点移到焦点顺序里的下一个元素。
    FocusNext,
    /// 把焦点移到焦点顺序里的上一个元素。
    FocusPrevious,
    /// 激活当前焦点上的元素。
    Activate,
    /// 关闭当前焦点所在的层。
    Dismiss,
    /// 沿着这个方向移动内容。
    ///
    /// 方向键在列表里移动光标、在页面里滚动视口，都是同一个方向意图：
    /// 移动的是哪个模型由表面自己的模型决定，shell 只规定方向。
    Scroll(ScrollDirection),
}

impl KeyIntent {
    /// shell 的全部意图。
    pub const ALL: [Self; 8] = [
        Self::FocusNext,
        Self::FocusPrevious,
        Self::Activate,
        Self::Dismiss,
        Self::Scroll(ScrollDirection::Up),
        Self::Scroll(ScrollDirection::Down),
        Self::Scroll(ScrollDirection::Left),
        Self::Scroll(ScrollDirection::Right),
    ];

    /// 该意图的方向；不是方向意图时没有方向。
    pub const fn scroll_direction(self) -> Option<ScrollDirection> {
        match self {
            Self::Scroll(direction) => Some(direction),
            Self::FocusNext | Self::FocusPrevious | Self::Activate | Self::Dismiss => None,
        }
    }

    /// 该意图是否移动焦点。
    pub const fn moves_focus(self) -> bool {
        matches!(self, Self::FocusNext | Self::FocusPrevious)
    }
}

/// 一次按键的导航意图。
///
/// 对 shell 没有意义的按键 —— 可打印字符、字段自己的退格 —— 什么意图都
/// 没有，因此表面不可能把它变成一个。
///
/// Tab 是键盘的焦点键，方向键是移动键：两者是不同的意图，因此焦点顺序
/// 与列表光标不会互相冒充。
pub fn key_intent(key: ShellKey) -> Option<KeyIntent> {
    match key {
        ShellKey::Tab => Some(KeyIntent::FocusNext),
        ShellKey::ShiftTab => Some(KeyIntent::FocusPrevious),
        ShellKey::Enter => Some(KeyIntent::Activate),
        ShellKey::Escape => Some(KeyIntent::Dismiss),
        ShellKey::ArrowUp => Some(KeyIntent::Scroll(ScrollDirection::Up)),
        ShellKey::ArrowDown => Some(KeyIntent::Scroll(ScrollDirection::Down)),
        ShellKey::ArrowLeft => Some(KeyIntent::Scroll(ScrollDirection::Left)),
        ShellKey::ArrowRight => Some(KeyIntent::Scroll(ScrollDirection::Right)),
        ShellKey::Backspace | ShellKey::Character(_) => None,
    }
}

/// 受保护字段的组合阶段（EFFECTS-010 #36）。
///
/// 四个阶段就是一次输入法会话在字段上的完整生命周期：字段空闲、输入法
/// 组合、组合被提交或被取消，然后字段结算回空闲。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FieldComposition {
    /// 空闲：字段显示模型的文本，发布值可以写进去。
    #[default]
    Idle,
    /// 组合中：字段属于输入法，发布值必须被抑制。
    Composing,
    /// 已提交：字段显示用户刚确认的文本，它比组合期间到达的发布值更新。
    Committed,
    /// 已取消：字段回滚到组合之前的文本，也就是模型的文本。
    Cancelled,
}

impl FieldComposition {
    /// 字段组合阶段的全部取值。
    pub const ALL: [Self; 4] = [
        Self::Idle,
        Self::Composing,
        Self::Committed,
        Self::Cancelled,
    ];

    /// 此刻发布值能不能写进字段。
    ///
    /// 空闲与已取消的字段是模型的字段，发布值可以写；组合中的字段属于
    /// 输入法，刚提交完的字段显示着用户比发布值更新的文本，两者都不能被
    /// 覆盖。
    pub const fn admits_snapshot(self) -> bool {
        match self {
            Self::Idle | Self::Cancelled => true,
            Self::Composing | Self::Committed => false,
        }
    }

    /// 输入法此刻是否拥有字段。
    pub const fn is_composing(self) -> bool {
        matches!(self, Self::Composing)
    }

    /// 组合已经结束、但字段还没有结算。
    pub const fn has_ended(self) -> bool {
        matches!(self, Self::Committed | Self::Cancelled)
    }

    /// 共享组合动作到达之后，字段进入的阶段。
    ///
    /// 转移覆盖共享词汇的全部动作，因此字段的阶段不可能和字段的文本各说
    /// 各话：动作是提交就是已提交，是取消就是已取消。
    pub fn transition(self, action: &ImeCompositionAction) -> Self {
        match action {
            ImeCompositionAction::Begin | ImeCompositionAction::UpdatePreedit(_) => Self::Composing,
            ImeCompositionAction::Commit(_) => Self::Committed,
            ImeCompositionAction::Cancel => Self::Cancelled,
        }
    }
}

/// 一份想要写进受保护字段的发布值，shell 怎么处理它（EFFECTS-010 #36）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotAdmission {
    /// 发布值可以写进字段。
    Apply,
    /// 发布值不能碰字段：此刻字段不属于模型。
    Suppress,
}

/// 让热重载不能覆盖用户正在输入的字段的保护闸门（EFFECTS-010 #36）。
///
/// 闸门只回答一个问题：*此刻这份发布值能不能写进这个字段*。答案由字段的
/// 归属决定：
///
/// * 空闲 —— 字段显示模型的文本，发布值可以写；
/// * 组合中 —— 输入法拥有字段，发布值被抑制；
/// * 已提交 —— 字段显示用户刚确认的文本，它比组合期间到达的发布值更新，
///   所以发布值在字段结算之前一直被抑制；
/// * 已取消 —— 字段回滚到模型的文本，发布值又可以写。
///
/// 组合的词汇是共享的：表面把同一份 [`ImeCompositionAction`] 既交给字段
/// 也交给闸门，因此“谁拥有字段”只有一个答案。
#[derive(Clone, Debug, Default)]
pub struct CompositionGuard {
    phase: FieldComposition,
    suppressed: u32,
}

impl CompositionGuard {
    /// 一道放行所有发布值的闸门。
    pub const fn new() -> Self {
        Self {
            phase: FieldComposition::Idle,
            suppressed: 0,
        }
    }

    /// 字段当前的组合阶段。
    pub const fn phase(&self) -> FieldComposition {
        self.phase
    }

    /// 输入法此刻是否拥有字段。
    pub const fn is_composing(&self) -> bool {
        self.phase.is_composing()
    }

    /// 此刻发布值能不能写进字段。
    pub const fn admits_snapshot(&self) -> bool {
        self.phase.admits_snapshot()
    }

    /// 闸门一共抑制了多少份发布值。
    pub const fn suppressed(&self) -> u32 {
        self.suppressed
    }

    /// 让一份发布值进门。
    ///
    /// 输入法拥有字段时发布值被抑制：模型保住自己的文本，字段保住用户正在
    /// 写的文本。被抑制的次数被记下来，因此“模型没有强行刷新文本框”这件
    /// 事是可以被看见的，而不是只能靠界面上的现象推断。想只问不记，用
    /// [`CompositionGuard::admits_snapshot`]。
    pub fn admit(&mut self) -> SnapshotAdmission {
        match self.phase {
            FieldComposition::Idle | FieldComposition::Cancelled => SnapshotAdmission::Apply,
            FieldComposition::Composing | FieldComposition::Committed => {
                self.suppressed = self.suppressed.saturating_add(1);
                SnapshotAdmission::Suppress
            }
        }
    }

    /// 喂给闸门表面已经应用到字段上的那一份共享组合动作。
    pub fn apply(&mut self, action: &ImeCompositionAction) -> FieldComposition {
        self.phase = self.phase.transition(action);
        self.phase
    }

    /// 结算字段：组合已经结束（提交、取消或被打断），字段重新属于模型。
    ///
    /// 提交之后必须结算：字段在结算之前一直显示用户刚确认的文本，发布值
    /// 也一直被抑制；结算之后发布值才重新写得进字段。
    pub fn settle(&mut self) -> FieldComposition {
        self.phase = FieldComposition::Idle;
        self.phase
    }
}

#[cfg(test)]
#[path = "input_intent_tests.rs"]
mod tests;
