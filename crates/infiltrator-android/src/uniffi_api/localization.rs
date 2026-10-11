use infiltrator_shared::i18n_interpolator::localize;

/// Shared copy keys used by the native Android routing-intent surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidRoutingIntentCopyKey {
    Title,
    ChangeAction,
    FollowRules,
    RouteAllThroughProxy,
    BypassProxy,
    Updating,
    Unsupported,
    Unavailable,
}

impl AndroidRoutingIntentCopyKey {
    const fn shared_key(self) -> &'static str {
        match self {
            Self::Title => "routing_intent_title",
            Self::ChangeAction => "routing_intent_change_action",
            Self::FollowRules => "routing_intent_follow_rules",
            Self::RouteAllThroughProxy => "routing_intent_route_all_through_proxy",
            Self::BypassProxy => "routing_intent_bypass_proxy",
            Self::Updating => "routing_intent_updating",
            Self::Unsupported => "routing_intent_unsupported",
            Self::Unavailable => "routing_intent_unavailable",
        }
    }
}

/// Return a routing-intent label from the shared localization catalogue.
#[uniffi::export]
pub fn android_routing_intent_copy(key: AndroidRoutingIntentCopyKey, language: String) -> String {
    localize(&language, key.shared_key(), &[])
}

/// Format an engine-owned intent with its adapter-authored extension identity.
#[uniffi::export]
pub fn android_engine_defined_routing_intent_copy(
    language: String,
    extension_id: String,
) -> String {
    localize(
        &language,
        "routing_intent_engine_defined",
        &[("extension", extension_id)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_routing_copy_uses_the_shared_bilingual_catalogue() {
        assert_eq!(
            android_routing_intent_copy(AndroidRoutingIntentCopyKey::FollowRules, "en-US".into()),
            "Follow rules",
        );
        assert_eq!(
            android_routing_intent_copy(AndroidRoutingIntentCopyKey::FollowRules, "zh-CN".into()),
            "遵循分流规则",
        );
        assert_eq!(
            android_engine_defined_routing_intent_copy("en-US".into(), "script".into()),
            "Engine-defined: script",
        );
        assert_eq!(
            android_engine_defined_routing_intent_copy("zh-CN".into(), "script".into()),
            "引擎专属：script",
        );
    }
}
