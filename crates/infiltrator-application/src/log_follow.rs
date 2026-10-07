//! Pure viewport policy shared by native log viewers; locking never stops log ingestion.
#[derive(Clone, Debug)]
pub struct LogFollowState {
    locked: bool,
    at_end: bool,
    offset: Option<f32>,
}
impl Default for LogFollowState {
    fn default() -> Self {
        Self {
            locked: false,
            at_end: true,
            offset: None,
        }
    }
}
impl LogFollowState {
    pub fn should_follow(&self) -> bool {
        !self.locked && self.at_end
    }
    pub fn toggle_follow(&mut self) {
        if self.should_follow() {
            self.locked = true;
        } else {
            self.locked = false;
            self.at_end = true;
        }
    }
    pub fn label_key(&self) -> &'static str {
        if self.should_follow() {
            "logs_scroll_lock"
        } else {
            "logs_scroll_resume"
        }
    }
    pub fn observe_viewport(&mut self, offset: f32, content: f32, viewport: f32) {
        if !offset.is_finite()
            || !content.is_finite()
            || !viewport.is_finite()
            || viewport < 0.0
            || content < 0.0
        {
            return;
        }
        let extent = (content - viewport).max(0.0);
        let offset = offset.clamp(0.0, extent);
        // A larger content extent without a scroll move is a new record or resize,
        // not evidence that the user scrolled away from the bottom.
        if self
            .offset
            .is_some_and(|previous| (previous - offset).abs() > 0.5)
        {
            self.at_end = extent - offset <= 1.0;
        }
        self.offset = Some(offset);
    }
    pub fn restored_offset(&self, extent: f32) -> f32 {
        if self.should_follow() {
            extent.max(0.0)
        } else {
            self.offset.unwrap_or_default().clamp(0.0, extent.max(0.0))
        }
    }
}
#[cfg(test)]
#[path = "log_follow_tests.rs"]
mod tests;
