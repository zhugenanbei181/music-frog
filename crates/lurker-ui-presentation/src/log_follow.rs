//! Pure viewport policy shared by native log viewers; locking never stops log ingestion.

#[derive(Clone, Debug, Default)]
pub struct LogFollowState {
    mode: FollowMode,
}

#[derive(Clone, Copy, Debug)]
enum FollowMode {
    Following { offset: Option<f32> },
    Paused { offset: Option<f32> },
    Away { offset: f32 },
}

impl Default for FollowMode {
    fn default() -> Self {
        Self::Following { offset: None }
    }
}

impl LogFollowState {
    pub fn should_follow(&self) -> bool {
        matches!(self.mode, FollowMode::Following { .. })
    }

    pub fn toggle_follow(&mut self) {
        self.mode = match self.mode {
            FollowMode::Following { offset } => FollowMode::Paused { offset },
            FollowMode::Paused { offset } => FollowMode::Following { offset },
            FollowMode::Away { offset } => FollowMode::Following {
                offset: Some(offset),
            },
        };
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
        self.mode = match self.mode {
            FollowMode::Following { offset: previous } => {
                if moved(previous, offset) && !is_at_end(extent, offset) {
                    FollowMode::Away { offset }
                } else {
                    FollowMode::Following {
                        offset: Some(offset),
                    }
                }
            }
            FollowMode::Paused { .. } => FollowMode::Paused {
                offset: Some(offset),
            },
            FollowMode::Away { offset: previous } => {
                if (previous - offset).abs() > 0.5 && is_at_end(extent, offset) {
                    FollowMode::Following {
                        offset: Some(offset),
                    }
                } else {
                    FollowMode::Away { offset }
                }
            }
        };
    }

    pub fn restored_offset(&self, extent: f32) -> f32 {
        let extent = extent.max(0.0);
        match self.mode {
            FollowMode::Following { .. } => extent,
            FollowMode::Paused { offset } => offset.unwrap_or_default().clamp(0.0, extent),
            FollowMode::Away { offset } => offset.clamp(0.0, extent),
        }
    }
}

fn moved(previous: Option<f32>, offset: f32) -> bool {
    previous.is_some_and(|previous| (previous - offset).abs() > 0.5)
}

fn is_at_end(extent: f32, offset: f32) -> bool {
    extent - offset <= 1.0
}

#[cfg(test)]
#[path = "log_follow_tests.rs"]
mod tests;
