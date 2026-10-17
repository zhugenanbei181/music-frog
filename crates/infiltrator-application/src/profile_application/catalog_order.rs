use super::ProfileApplication;
use infiltrator_contract::profile_catalog_order::{
    ProfileCatalogOrderMutation, ProfileCatalogOrderRequest, ProfileCatalogSnapshot,
};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::profile_document_identity::ProfileId;

impl ProfileApplication {
    /// Return the complete engine-scoped identity order currently visible to
    /// the profile catalog.
    pub async fn profile_catalog_order_snapshot(&self) -> Result<ProfileCatalogSnapshot, Failure> {
        let profiles = self.store.list_profiles().await.map_err(Failure::from)?;
        let ordered_profile_ids = profiles
            .into_iter()
            .map(|profile| {
                ProfileId::new(profile.name).map_err(|error| {
                    Failure::new(ErrorCode::Configuration, error.to_string(), false)
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ProfileCatalogSnapshot::new(
            self.store.engine_id(),
            ordered_profile_ids,
        ))
    }

    /// Publish an exact permutation only when the full catalog order still
    /// matches the snapshot observed by the caller.
    pub async fn set_profile_catalog_order(
        &self,
        request: ProfileCatalogOrderRequest,
    ) -> Result<ProfileCatalogOrderMutation, Failure> {
        request
            .validate_for_engine(&self.store.engine_id())
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        self.store
            .set_profile_catalog_order(request)
            .await
            .map_err(Failure::from)
    }
}
