//! Global shortcut use-cases over the runtime-neutral settings store port.
//!
//! The registry itself (chord grammar, product defaults, conflict rules) is
//! the shared contract; this layer owns the persistence round trip and turns
//! a conflicting capture into a typed [`Failure`] the surfaces can toast.

use crate::settings_application::SettingsApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::shortcuts::{ShortcutAction, ShortcutChord, ShortcutRegistry};
use std::sync::Arc;

#[derive(Clone)]
pub struct ShortcutApplication {
    settings: SettingsApplication,
}

impl ShortcutApplication {
    pub fn new(settings: SettingsApplication) -> Self {
        Self { settings }
    }

    pub fn from_store(store: Arc<dyn infiltrator_ports::settings_store::SettingsStore>) -> Self {
        Self::new(SettingsApplication::new(store))
    }

    /// The registry as stored, repaired against the product defaults (an
    /// empty list means "product defaults", an unknown action is dropped).
    pub async fn registry(&self) -> Result<ShortcutRegistry, Failure> {
        let settings = self.settings.load().await?;
        Ok(ShortcutRegistry::from_bindings(settings.shortcuts).normalize())
    }

    /// Capture a chord for one action. A chord already owned by another
    /// action is rejected with the conflict message; otherwise the binding is
    /// persisted and the updated registry returned.
    pub async fn capture(
        &self,
        action: ShortcutAction,
        chord: ShortcutChord,
    ) -> Result<ShortcutRegistry, Failure> {
        let mut registry = self.registry().await?;
        if let Err(conflict) = registry.bind_or_replace(action, chord) {
            return Err(invalid_input(format!(
                "shortcut conflict: {}",
                conflict.message
            )));
        }
        self.persist(&registry).await?;
        Ok(registry)
    }

    /// Enable or disable one action's binding.
    pub async fn set_enabled(
        &self,
        action: ShortcutAction,
        enabled: bool,
    ) -> Result<ShortcutRegistry, Failure> {
        let mut registry = self.registry().await?;
        if !registry.set_enabled(action, enabled) {
            return Err(invalid_input(format!(
                "shortcut action {} is not bound",
                action.id()
            )));
        }
        self.persist(&registry).await?;
        Ok(registry)
    }

    /// Restore one action's product default chord and re-enable it.
    pub async fn reset_action(&self, action: ShortcutAction) -> Result<ShortcutRegistry, Failure> {
        let mut registry = self.registry().await?;
        if let Err(conflict) = registry.reset_action(action) {
            return Err(invalid_input(format!(
                "shortcut conflict: {}",
                conflict.message
            )));
        }
        self.persist(&registry).await?;
        Ok(registry)
    }

    /// Restore the whole product default set.
    pub async fn reset_all(&self) -> Result<ShortcutRegistry, Failure> {
        let mut registry = self.registry().await?;
        registry.reset_all();
        self.persist(&registry).await?;
        Ok(registry)
    }

    async fn persist(&self, registry: &ShortcutRegistry) -> Result<(), Failure> {
        let bindings = registry.bindings().to_vec();
        self.settings
            .update(move |settings| settings.shortcuts = bindings)
            .await
    }
}

fn invalid_input(message: String) -> Failure {
    Failure::new(ErrorCode::InvalidInput, message, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_domain::settings::AppSettings;
    use infiltrator_ports::error::PortError;
    use infiltrator_ports::settings_store::SettingsStore;
    use std::sync::Mutex;

    #[derive(Default)]
    struct MemorySettingsStore {
        settings: Mutex<AppSettings>,
    }

    #[async_trait]
    impl SettingsStore for MemorySettingsStore {
        async fn load(&self) -> Result<AppSettings, PortError> {
            Ok(self.settings.lock().expect("lock").clone())
        }

        async fn load_hydrated(&self) -> Result<AppSettings, PortError> {
            self.load().await
        }

        async fn save(&self, settings: &AppSettings) -> Result<(), PortError> {
            *self.settings.lock().expect("lock") = settings.clone();
            Ok(())
        }
    }

    fn application() -> ShortcutApplication {
        ShortcutApplication::from_store(Arc::new(MemorySettingsStore::default()))
    }

    #[test]
    fn empty_settings_seed_the_product_defaults() {
        let application = application();
        let registry = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime")
            .block_on(application.registry())
            .expect("registry");
        assert_eq!(
            registry.bindings().len(),
            ShortcutAction::ALL.len(),
            "an empty settings file means the product defaults"
        );
    }

    #[test]
    fn capture_persists_and_rejects_conflicts() {
        let application = application();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        let chord = ShortcutChord::new(
            "Y",
            infiltrator_contract::shortcuts::KeyModifiers::ctrl_alt(),
        );
        let updated = runtime
            .block_on(application.capture(ShortcutAction::ToggleTun, chord.clone()))
            .expect("capture");
        assert_eq!(
            updated.get(ShortcutAction::ToggleTun).expect("bound").chord,
            chord
        );

        let conflict = runtime
            .block_on(application.capture(ShortcutAction::ToggleMiniHud, chord))
            .expect_err("conflict");
        assert_eq!(conflict.code, ErrorCode::InvalidInput);
        assert!(conflict.message.contains("shortcut conflict"));
        assert!(!conflict.retryable);

        // The rejected capture did not touch the stored binding.
        let reloaded = runtime.block_on(application.registry()).expect("registry");
        assert_eq!(
            reloaded
                .get(ShortcutAction::ToggleMiniHud)
                .expect("bound")
                .chord,
            ShortcutChord::new(
                "M",
                infiltrator_contract::shortcuts::KeyModifiers::ctrl_alt()
            )
        );
    }

    #[test]
    fn enable_disable_and_reset_round_trip_through_the_store() {
        let application = application();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        let disabled = runtime
            .block_on(application.set_enabled(ShortcutAction::ToggleTun, false))
            .expect("disable");
        assert!(!disabled.is_active(ShortcutAction::ToggleTun));

        let reset = runtime
            .block_on(application.reset_action(ShortcutAction::ToggleTun))
            .expect("reset");
        assert!(reset.is_active(ShortcutAction::ToggleTun));

        let reset_all = runtime
            .block_on(application.reset_all())
            .expect("reset all");
        assert!(reset_all.detect_conflicts().is_empty());
        assert_eq!(reset_all.bindings().len(), ShortcutAction::ALL.len());
    }

    #[test]
    fn a_stored_custom_chord_blocks_later_captures() {
        let application = application();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        // Seed a stored binding that occupies the palette default chord.
        runtime
            .block_on(application.settings.update(|settings| {
                settings
                    .shortcuts
                    .push(infiltrator_contract::shortcuts::ShortcutBinding::new(
                        ShortcutAction::CycleTheme,
                        ShortcutChord::ctrl_key("K"),
                    ));
            }))
            .expect("seed");
        let registry = runtime.block_on(application.registry()).expect("registry");
        assert!(registry.detect_conflicts().is_empty());
        assert_eq!(
            registry
                .get(ShortcutAction::CycleTheme)
                .expect("custom binding")
                .chord,
            ShortcutChord::ctrl_key("K")
        );
        // The palette default is left unbound rather than silently shadowing
        // the stored custom chord.
        assert!(registry.get(ShortcutAction::OpenCommandPalette).is_none());
        // A later capture of that chord is refused.
        let conflict = runtime
            .block_on(application.capture(ShortcutAction::ToggleTun, ShortcutChord::ctrl_key("K")))
            .expect_err("conflict");
        assert_eq!(conflict.code, ErrorCode::InvalidInput);
        assert!(conflict.message.contains("cycle_theme"));
    }
}
