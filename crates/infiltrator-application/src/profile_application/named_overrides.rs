//! ProfileApplication methods for Named Overrides (SLINT-004).

use super::ProfileApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::named_override::{
    NamedOverride, NamedOverrideCatalog, NamedOverrideId,
};

impl ProfileApplication {
    pub async fn load_named_override_catalog(&self) -> Result<NamedOverrideCatalog, Failure> {
        self.store.load_named_override_catalog().await.map_err(Failure::from)
    }

    pub async fn save_named_override_catalog(
        &self,
        catalog: &NamedOverrideCatalog,
    ) -> Result<(), Failure> {
        self.store.save_named_override_catalog(catalog).await.map_err(Failure::from)
    }

    pub async fn create_named_override(
        &self,
        name: String,
        content: String,
    ) -> Result<NamedOverrideId, Failure> {
        let mut catalog = self.load_named_override_catalog().await?;
        let id_str = name
            .to_lowercase()
            .replace(' ', "-")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect::<String>();
        let id = NamedOverrideId::new(if id_str.is_empty() { "override".into() } else { id_str })
            .map_err(|err| Failure::new(ErrorCode::InvalidInput, err.to_string(), false))?;
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        let item = NamedOverride::new(id.clone(), name, content, now)
            .map_err(|err| Failure::new(ErrorCode::InvalidInput, err.to_string(), false))?;
        catalog
            .upsert_override(catalog.revision, item)
            .map_err(|err| Failure::new(ErrorCode::NotReady, err.to_string(), true))?;
        self.save_named_override_catalog(&catalog).await?;
        Ok(id)
    }

    pub async fn update_named_override(
        &self,
        id: NamedOverrideId,
        name: Option<String>,
        content: Option<String>,
    ) -> Result<(), Failure> {
        let mut catalog = self.load_named_override_catalog().await?;
        let existing = catalog
            .get_override(&id)
            .ok_or_else(|| Failure::new(ErrorCode::InvalidInput, "Override not found", false))?
            .clone();
        let new_name = name.unwrap_or(existing.name);
        let new_content = content.unwrap_or(existing.content);
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        let updated = NamedOverride::new(id, new_name, new_content, now)
            .map_err(|err| Failure::new(ErrorCode::InvalidInput, err.to_string(), false))?;
        catalog
            .upsert_override(catalog.revision, updated)
            .map_err(|err| Failure::new(ErrorCode::NotReady, err.to_string(), true))?;
        self.save_named_override_catalog(&catalog).await
    }

    pub async fn delete_named_override(&self, id: NamedOverrideId) -> Result<(), Failure> {
        let mut catalog = self.load_named_override_catalog().await?;
        catalog
            .delete_override(catalog.revision, &id)
            .map_err(|err| Failure::new(ErrorCode::NotReady, err.to_string(), true))?;
        self.save_named_override_catalog(&catalog).await
    }

    pub async fn set_profile_override_binding(
        &self,
        profile_id: String,
        override_id: NamedOverrideId,
        enabled: bool,
    ) -> Result<(), Failure> {
        let mut catalog = self.load_named_override_catalog().await?;
        catalog
            .set_profile_binding(catalog.revision, &profile_id, &override_id, enabled)
            .map_err(|err| Failure::new(ErrorCode::NotReady, err.to_string(), true))?;
        self.save_named_override_catalog(&catalog).await
    }

    pub async fn reorder_profile_overrides(
        &self,
        profile_id: String,
        order: Vec<NamedOverrideId>,
    ) -> Result<(), Failure> {
        let mut catalog = self.load_named_override_catalog().await?;
        catalog
            .reorder_profile_bindings(catalog.revision, &profile_id, &order)
            .map_err(|err| Failure::new(ErrorCode::NotReady, err.to_string(), true))?;
        self.save_named_override_catalog(&catalog).await
    }
}
