//! An unsent form remains owned by the source observed when editing began.
use crate::rule_list_editor::RuleListEditor;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_domain::rules::edit::DEFAULT_RULE_TARGET;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn default_form_target(editor: &RuleListEditor) -> String {
    let Some(base) = editor.base.as_ref() else {
        return String::new();
    };
    if base
        .targets
        .iter()
        .any(|target| target == DEFAULT_RULE_TARGET)
    {
        DEFAULT_RULE_TARGET.to_owned()
    } else {
        base.targets
            .iter()
            .find(|target| target.as_str() == "DIRECT")
            .or_else(|| base.targets.first())
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum RuleFormBinding {
    #[default]
    Unbound,
    Clean {
        source: RuleSourceIdentity,
    },
    Edited {
        source: RuleSourceIdentity,
    },
    EditedWithoutSource,
}
impl RuleFormBinding {
    pub fn observe(&mut self, editor: &RuleListEditor) -> bool {
        if matches!(self, Self::Edited { .. } | Self::EditedWithoutSource) {
            return false;
        }
        let source = editor.base.as_ref().map(|base| base.source.clone());
        let changed = self.source() != source.as_ref();
        *self = match source {
            Some(source) => Self::Clean { source },
            None => Self::Unbound,
        };
        changed
    }
    pub fn edit(&mut self, editor: &RuleListEditor) {
        self.observe(editor);
        *self = match self.clone() {
            Self::Clean { source } => Self::Edited { source },
            Self::Unbound => Self::EditedWithoutSource,
            edited @ (Self::Edited { .. } | Self::EditedWithoutSource) => edited,
        };
    }
    pub fn reset(&mut self, editor: &RuleListEditor) {
        *self = match editor.base.as_ref() {
            Some(base) => Self::Clean {
                source: base.source.clone(),
            },
            None => Self::Unbound,
        };
    }
    pub fn current(&self, editor: &RuleListEditor) -> bool {
        editor.editable()
            && self
                .source()
                .zip(editor.base.as_ref())
                .is_some_and(|(source, base)| source == &base.source)
    }
    pub fn require_current(&self, editor: &RuleListEditor) -> Result<(), Failure> {
        if self.current(editor) {
            Ok(())
        } else {
            Err(Failure::new(
                ErrorCode::NotReady,
                "This unsent rule form belongs to an unavailable or changed source; discard it before editing the current document",
                true,
            ))
        }
    }
    pub fn status(&self, editor: &RuleListEditor, locale: &str) -> String {
        if editor.read_failure().is_none()
            && !editor.source_changed()
            && self
                .source()
                .zip(editor.base.as_ref())
                .is_some_and(|(source, base)| source == &base.source)
        {
            return String::new();
        }
        interpolate(
            Lang(locale).tr("rules_form_source_changed").as_ref(),
            &[(
                "profile",
                self.source().map_or("—", |source| source.profile.as_str()),
            )],
        )
    }

    fn source(&self) -> Option<&RuleSourceIdentity> {
        match self {
            Self::Unbound | Self::EditedWithoutSource => None,
            Self::Clean { source } | Self::Edited { source } => Some(source),
        }
    }
}

#[cfg(test)]
#[path = "rule_form_binding_test.rs"]
mod tests;
