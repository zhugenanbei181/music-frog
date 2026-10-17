//! Per-app routing use-cases over the routing persistence port.

use infiltrator_domain::app_routing::{
    AppRoutingConfig, AppRoutingMode, AppRoutingPackageSelection, AppRoutingRule,
};
use infiltrator_ports::app_routing_store::AppRoutingStore;
use lurker_core_contract::error::{ErrorCode, Failure};
use std::sync::Arc;

#[derive(Clone)]
pub struct RoutingApplication {
    store: Arc<dyn AppRoutingStore>,
}

impl RoutingApplication {
    pub fn new(store: Arc<dyn AppRoutingStore>) -> Self {
        Self { store }
    }

    pub fn load(&self) -> Result<AppRoutingConfig, Failure> {
        self.store.load().map_err(Failure::from)
    }

    pub fn save(&self, config: &AppRoutingConfig) -> Result<(), Failure> {
        self.store.save(config).map_err(Failure::from)
    }

    pub fn set_mode(&self, mode: AppRoutingMode) -> Result<(), Failure> {
        let mut config = self.load()?;
        config.mode = mode;
        self.save(&config)
    }

    pub fn set_package_selection(
        &self,
        package: &str,
        selection: AppRoutingPackageSelection,
    ) -> Result<(), Failure> {
        self.set_packages(&[package.to_owned()], selection)
    }

    /// Applies a selection state to a package group in one persisted update.
    /// Existing selections outside the group and explicit package rules remain intact.
    pub fn set_packages(
        &self,
        packages: &[String],
        selection: AppRoutingPackageSelection,
    ) -> Result<(), Failure> {
        let packages = packages
            .iter()
            .map(|package| package.trim())
            .collect::<Vec<_>>();
        if packages.iter().any(|package| package.is_empty()) {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "package name is empty",
                false,
            ));
        }

        let mut config = self.load()?;
        for package in packages {
            match selection {
                AppRoutingPackageSelection::Selected => {
                    config.packages.insert(package.to_string());
                }
                AppRoutingPackageSelection::Unselected => {
                    config.packages.remove(package);
                }
            }
        }
        self.save(&config)
    }

    pub fn set_rule(&self, package: &str, rule: AppRoutingRule) -> Result<(), Failure> {
        let package = package.trim();
        if package.is_empty() {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "package name is empty",
                false,
            ));
        }
        let mut config = self.load()?;
        config.rules.insert(package.to_string(), rule);
        self.save(&config)
    }

    pub fn allowed_packages(&self) -> Result<Option<Vec<String>>, Failure> {
        Ok(self.load()?.get_allowed_packages())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_domain::app_routing::AppRoutingRule;
    use infiltrator_ports::app_routing_store::AppRoutingStore;
    use infiltrator_ports::error::PortError;
    use std::collections::{HashMap, HashSet};
    use std::sync::Mutex;

    struct MemoryRoutingStore(Mutex<AppRoutingConfig>);

    impl AppRoutingStore for MemoryRoutingStore {
        fn load(&self) -> Result<AppRoutingConfig, PortError> {
            Ok(self.0.lock().expect("routing store lock").clone())
        }

        fn save(&self, config: &AppRoutingConfig) -> Result<(), PortError> {
            *self.0.lock().expect("routing store lock") = config.clone();
            Ok(())
        }
    }

    #[test]
    fn package_group_selection_preserves_other_packages_and_explicit_rules() {
        let store = Arc::new(MemoryRoutingStore(Mutex::new(AppRoutingConfig {
            mode: AppRoutingMode::ProxySelected,
            packages: HashSet::from(["retained.package".to_owned()]),
            rules: HashMap::from([("explicit.package".to_owned(), AppRoutingRule::Direct)]),
        })));
        let application = RoutingApplication::new(store.clone());

        application
            .set_packages(
                &[
                    " selected.package ".to_owned(),
                    "selected.package".to_owned(),
                ],
                AppRoutingPackageSelection::Selected,
            )
            .expect("select the requested package group");

        let selected = application.load().expect("read selected packages");
        assert!(selected.packages.contains("retained.package"));
        assert!(selected.packages.contains("selected.package"));
        assert_eq!(
            selected.rules.get("explicit.package"),
            Some(&AppRoutingRule::Direct),
        );

        application
            .set_packages(
                &["selected.package".to_owned()],
                AppRoutingPackageSelection::Unselected,
            )
            .expect("clear only the requested package");

        let cleared = application.load().expect("read cleared packages");
        assert!(cleared.packages.contains("retained.package"));
        assert!(!cleared.packages.contains("selected.package"));
        assert_eq!(
            cleared.rules.get("explicit.package"),
            Some(&AppRoutingRule::Direct),
        );
    }

    #[test]
    fn package_group_selection_rejects_empty_ids_before_writing() {
        let initial = AppRoutingConfig {
            mode: AppRoutingMode::BypassSelected,
            packages: HashSet::from(["retained.package".to_owned()]),
            ..AppRoutingConfig::default()
        };
        let store = Arc::new(MemoryRoutingStore(Mutex::new(initial.clone())));
        let application = RoutingApplication::new(store.clone());

        let failure = application
            .set_packages(
                &["valid.package".to_owned(), "   ".to_owned()],
                AppRoutingPackageSelection::Selected,
            )
            .expect_err("empty package IDs are rejected");

        assert_eq!(failure.code, ErrorCode::InvalidInput);
        assert_eq!(
            store.0.lock().expect("routing store lock").packages,
            initial.packages
        );
    }
}
