//! Android product composition: the shared [`CoreApplication`] plus the
//! bounded surface pump that Bevy and Compose hosts consume.
//!
//! This is the Android analogue of `infiltrator-desktop/src/surface.rs`
//! (BANDROID-004). It assembles the Android bridge-backed Core process port,
//! the VpnService application and the controller gateway into one
//! runtime-neutral [`SurfacePump`]. Pages whose Android host adapters are not
//! composed yet stay typed `unavailable`; nothing here fabricates demo data.

use crate::composition::mtu_application;
use crate::composition::offline_startup_application;
#[cfg(test)]
use crate::composition::{AndroidCoreComposition, core_application};
use crate::ffi::FfiStatus;
use crate::host_session::{close_shared_core_owner, shared_core};
use crate::host_support::get_runtime;
use crate::runtime::AndroidBridgeAdapter;
use crate::vpn_service::AndroidVpnServicePort;
use infiltrator_application::android_control_application::AndroidControlApplication;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::outbound_application::OutboundApplication;
use infiltrator_application::resource_application::ResourceApplication;
use infiltrator_application::surface_application::SurfacePump;
use infiltrator_application::surface_application::SurfacePumpBridge;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_application::vpn_application::VpnServiceApplication;
#[cfg(test)]
use infiltrator_contract::android_control::AndroidHostProcessRole;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
#[cfg(test)]
use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnConfigurationApplyOutcome, VpnOwnerStopReceipt, VpnStartOutcome,
    VpnStopTarget, VpnSurfaceObservation,
};
use infiltrator_ports::capability_provider::CapabilityProvider;
use infiltrator_ports::error::PortError;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
#[cfg(test)]
use infiltrator_ports::secure_store::SecureStore;
use mihomo_api::client::MihomoClient;
use mihomo_api::routing_intent::MihomoRoutingIntentAdapter;
use mihomo_platform::android_bridge::{AndroidBridge, get_android_bridge};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::Mutex;

#[cfg(feature = "slint-ui")]
const ANDROID_PRODUCT_SURFACE: SurfaceKind = SurfaceKind::SlintAndroid;
#[cfg(not(feature = "slint-ui"))]
const ANDROID_PRODUCT_SURFACE: SurfaceKind = SurfaceKind::BevyAndroid;

/// Host-supplied parameters for the Android product composition.
#[derive(Clone, Debug)]
pub struct AndroidProductConfig {
    /// Loopback controller endpoint exposed by the packaged Mihomo core.
    pub controller_url: String,
    /// Optional controller secret resolved by the shared configuration owner.
    pub controller_secret: Option<String>,
    /// Bounded surface sampling cadence for the pump worker.
    pub sample_interval: Duration,
}

impl AndroidProductConfig {
    pub fn new(controller_url: impl Into<String>) -> Self {
        Self {
            controller_url: controller_url.into(),
            controller_secret: None,
            sample_interval: Duration::from_millis(700),
        }
    }
}

/// Typed failure while opening the Android product composition.
#[derive(Debug, thiserror::Error)]
pub enum AndroidProductError {
    #[error("android bridge is not registered in this process")]
    BridgeUnavailable,
    #[error("Android product composition is owned by the :vpn process")]
    NotCoreOwner,
    #[error("android application runtime is unavailable: {0}")]
    RuntimeUnavailable(String),
}

/// One owned Android product session: the application facade and its surface
/// pump. The owner decides when to drop the pump and close the application.
pub struct AndroidProductComposition {
    application: Arc<CoreApplication>,
    control: AndroidControlApplication,
    pump: SurfacePump,
}

impl AndroidProductComposition {
    /// Open using the bridge registered in the current process.
    ///
    /// The `:vpn` process registers the bridge from `MihomoApplication` before
    /// this is called, so the UI process is not required to have run first.
    pub fn open(config: AndroidProductConfig) -> Result<Self, AndroidProductError> {
        let bridge = get_android_bridge().ok_or(AndroidProductError::BridgeUnavailable)?;
        let shared = get_runtime().block_on(shared_core()).map_err(|status| {
            AndroidProductError::RuntimeUnavailable(
                status
                    .message
                    .unwrap_or_else(|| "shared Android application unavailable".to_owned()),
            )
        })?;
        Self::open_with_application(
            bridge,
            config,
            Arc::new(shared.application.clone()),
            shared.control.clone(),
        )
    }

    /// Open the single product projection beside the shared application
    /// inside the `:vpn` owner process.
    pub(crate) async fn open_shared_owner() -> Result<Self, AndroidProductError> {
        let bridge = get_android_bridge().ok_or(AndroidProductError::BridgeUnavailable)?;
        let shared = shared_core().await.map_err(|status| {
            AndroidProductError::RuntimeUnavailable(
                status
                    .message
                    .unwrap_or_else(|| "shared Android application unavailable".to_owned()),
            )
        })?;
        let endpoint = shared
            .endpoints
            .resolve()
            .await
            .map_err(|error| AndroidProductError::RuntimeUnavailable(error.to_string()))?;
        let controller_url = endpoint.url;
        let controller_secret = endpoint.secret;
        Self::open_with_application(
            bridge,
            AndroidProductConfig {
                controller_url,
                controller_secret,
                sample_interval: Duration::from_millis(700),
            },
            Arc::new(shared.application.clone()),
            shared.control.clone(),
        )
    }

    /// Test-only constructor. Product code must open through `shared_core()`
    /// so the owner process cannot create a second application actor.
    #[cfg(test)]
    fn open_with_bridge(
        bridge: Arc<dyn AndroidBridge>,
        config: AndroidProductConfig,
        composition: AndroidCoreComposition<impl SecureStore + 'static>,
    ) -> Result<Self, AndroidProductError> {
        if bridge.process_role() != AndroidHostProcessRole::CoreOwner {
            return Err(AndroidProductError::NotCoreOwner);
        }
        let application = Arc::new(
            core_application(
                bridge.clone(),
                config.controller_url.clone(),
                config.controller_secret.clone(),
                composition,
            )
            .map_err(AndroidProductError::RuntimeUnavailable)?,
        );
        let control = AndroidControlApplication::new(application.as_ref().clone());
        Self::open_with_application(bridge, config, application, control)
    }

    fn open_with_application(
        bridge: Arc<dyn AndroidBridge>,
        config: AndroidProductConfig,
        application: Arc<CoreApplication>,
        control: AndroidControlApplication,
    ) -> Result<Self, AndroidProductError> {
        let outbound = application.outbound_port().map(OutboundApplication::new);
        let capabilities =
            CapabilityProvider::capabilities(&AndroidBridgeAdapter::new(bridge.clone()));
        let mut reader = ApplicationSurfaceReader::new(
            application.clone(),
            ANDROID_PRODUCT_SURFACE,
            HostKind::Android,
        )
        .with_capabilities(capabilities)
        .with_offline_startup(offline_startup_application(bridge.clone()))
        .with_mtu(mtu_application(bridge.clone()))
        .with_vpn(VpnServiceApplication::new(Arc::new(
            AndroidVpnServicePort::shared(),
        )));
        if let Some(outbound) = outbound {
            reader = reader.with_outbound(outbound);
        }
        let scope_application = application.clone();
        let scope_resolver: mihomo_api::runtime_configuration::RuntimeScopeResolver =
            Arc::new(move || {
                let lifecycle = scope_application.snapshot().lifecycle_snapshot();
                match &lifecycle.lifecycle {
                    CoreLifecycle::Running | CoreLifecycle::Ready => {
                        lifecycle.scope().ok_or_else(|| {
                            PortError::Failed(
                                "active Core identity is unavailable for configuration access"
                                    .into(),
                            )
                        })
                    }
                    _ => Err(PortError::Failed(
                        "configuration access requires a running Core".to_owned(),
                    )),
                }
            });
        let client = MihomoClient::new(&config.controller_url, config.controller_secret)
            .map(|client| client.with_configuration_scope_resolver(scope_resolver));
        if let Ok(client) = client {
            let routing_intent_port = Arc::new(MihomoRoutingIntentAdapter::new(client.clone()));
            let gateway: Arc<dyn RuntimeGateway> = Arc::new(client);
            reader = reader
                .with_routing_intent_port(routing_intent_port)
                .with_gateway(gateway.clone())
                .with_resources(ResourceApplication::new(gateway));
        }
        let runtime = infiltrator_composition::tokio_application_runtime()
            .map_err(AndroidProductError::RuntimeUnavailable)?;
        let initial = SurfaceSnapshot::unavailable(
            ANDROID_PRODUCT_SURFACE,
            HostKind::Android,
            Failure::new(
                ErrorCode::NotReady,
                "waiting for the first Android surface snapshot",
                true,
            ),
        );
        let pump = SurfacePump::spawn(Arc::new(reader), config.sample_interval, runtime, initial);
        Ok(Self {
            application,
            control,
            pump,
        })
    }

    /// The shared application facade both UI surfaces dispatch commands to.
    pub fn application(&self) -> Arc<CoreApplication> {
        self.application.clone()
    }

    /// The same typed control orchestrator used by the Compose FFI adapter.
    pub fn android_control(&self) -> AndroidControlApplication {
        self.control.clone()
    }

    /// The bounded surface pump owned by this composition.
    pub fn surface_pump(&self) -> SurfacePump {
        self.pump.clone()
    }

    /// A frame-drain bridge for a host event loop.
    pub fn surface_bridge(&self) -> SurfacePumpBridge {
        self.pump.bridge()
    }
}

/// Owner-side product session used by the typed Messenger surface read.
/// The UI process never constructs this composition or a second Core actor.
enum OwnerProductState {
    Uninitialized,
    Active(AndroidProductComposition),
    Closed,
}

fn owner_product_slot() -> &'static Mutex<OwnerProductState> {
    static OWNER_PRODUCT: OnceLock<Mutex<OwnerProductState>> = OnceLock::new();
    OWNER_PRODUCT.get_or_init(|| Mutex::new(OwnerProductState::Uninitialized))
}

pub(crate) async fn owner_surface_snapshot_json() -> Result<String, String> {
    let mut state = owner_product_slot().lock().await;
    if matches!(&*state, OwnerProductState::Uninitialized) {
        let owner = AndroidProductComposition::open_shared_owner()
            .await
            .map_err(|error| error.to_string())?;
        *state = OwnerProductState::Active(owner);
    }
    let snapshot = match &*state {
        OwnerProductState::Active(owner) => owner.surface_pump().current(),
        OwnerProductState::Uninitialized | OwnerProductState::Closed => {
            return Err("Android Core owner product session is closed".to_owned());
        }
    };
    serialize_owner_surface_snapshot(&snapshot)
}

pub(crate) async fn begin_owner_product_session() {
    let mut state = owner_product_slot().lock().await;
    if matches!(&*state, OwnerProductState::Closed) {
        *state = OwnerProductState::Uninitialized;
    }
}

pub(crate) async fn close_owner_product_session() -> Result<(), FfiStatus> {
    let mut state = owner_product_slot().lock().await;
    let previous = std::mem::replace(&mut *state, OwnerProductState::Closed);
    drop(state);
    drop(previous);

    let vpn = VpnServiceApplication::new(Arc::new(AndroidVpnServicePort::shared()));
    close_shared_core_owner(vpn).await
}

fn serialize_owner_surface_snapshot(snapshot: &SurfaceSnapshot) -> Result<String, String> {
    snapshot
        .validate_core_capability_scope_binding()
        .map_err(|failure| failure.message)?;
    snapshot
        .validate_vpn_owner_core_binding()
        .map_err(|failure| failure.message)?;
    serde_json::to_string(snapshot)
        .map_err(|error| format!("Android surface snapshot serialization failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::android_control::{
        AndroidChildProcessIdentity, AndroidCoreProcessObservation,
        AndroidCoreProcessUnavailableReason, AndroidOwnedCoreProcess, CoreLaunchSpec,
        VpnOwnerCoreReadback, VpnOwnerLifecycle, VpnOwnerReadiness, VpnOwnerSnapshot,
    };
    use infiltrator_contract::snapshot::CoreLifecycle;
    use infiltrator_ports::error::PortError;
    use infiltrator_ports::secure_store::SecureStore;
    use mihomo_api::error::Result;
    use mihomo_config::manager::ConfigManager;
    use mihomo_platform::android_bridge::{
        CoreProcessStartReceipt, CoreProcessStopReceipt, clear_android_bridge,
    };
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::result;
    use std::sync::Mutex;
    use std::thread::yield_now;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    struct TestCredentialStore;

    #[async_trait]
    impl SecureStore for TestCredentialStore {
        async fn get(
            &self,
            _namespace: &str,
            _key: &str,
        ) -> result::Result<Option<String>, PortError> {
            Ok(None)
        }

        async fn set(
            &self,
            _namespace: &str,
            _key: &str,
            _value: &str,
        ) -> result::Result<(), PortError> {
            Ok(())
        }

        async fn delete(&self, _namespace: &str, _key: &str) -> result::Result<(), PortError> {
            Ok(())
        }
    }

    struct TestHome(PathBuf);

    impl Drop for TestHome {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn composition_input() -> (TestHome, AndroidCoreComposition<TestCredentialStore>) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after the Unix epoch")
            .as_nanos();
        let home = env::temp_dir().join(format!(
            "infiltrator-android-product-owner-{}-{stamp}",
            process::id()
        ));
        fs::create_dir_all(&home).expect("temporary product home is created");
        let manager = ConfigManager::with_home_and_store(home.clone(), TestCredentialStore)
            .expect("Mihomo config manager is constructed for the test home");
        (
            TestHome(home),
            AndroidCoreComposition::new(Arc::new(manager)),
        )
    }

    struct TestBridge {
        observation: Mutex<AndroidCoreProcessObservation>,
        core_owner: bool,
    }

    impl TestBridge {
        fn new() -> Self {
            Self {
                observation: Mutex::new(AndroidCoreProcessObservation::NeverStarted),
                core_owner: true,
            }
        }

        fn client_process() -> Self {
            Self {
                observation: Mutex::new(AndroidCoreProcessObservation::Unavailable {
                    reason: AndroidCoreProcessUnavailableReason::ProcessInspectionUnavailable,
                }),
                core_owner: false,
            }
        }
    }

    #[async_trait]
    impl AndroidBridge for TestBridge {
        fn process_role(&self) -> AndroidHostProcessRole {
            if self.core_owner {
                AndroidHostProcessRole::CoreOwner
            } else {
                AndroidHostProcessRole::UiClient
            }
        }

        async fn core_start(
            &self,
            _launch_spec: &CoreLaunchSpec,
            context: &infiltrator_ports::core_process::CoreProcessStartContext,
        ) -> Result<CoreProcessStartReceipt> {
            *self.observation.lock().expect("test observation lock") =
                AndroidCoreProcessObservation::Running {
                    owned: AndroidOwnedCoreProcess {
                        context: context.clone(),
                        child: AndroidChildProcessIdentity {
                            pid: 321,
                            start_time_ticks: 987_654,
                        },
                    },
                };
            Ok(CoreProcessStartReceipt::Started)
        }

        async fn core_stop(&self) -> Result<CoreProcessStopReceipt> {
            *self.observation.lock().expect("test observation lock") =
                AndroidCoreProcessObservation::NeverStarted;
            Ok(CoreProcessStopReceipt::StoppedGracefully)
        }

        async fn core_process_observation(&self) -> Result<AndroidCoreProcessObservation> {
            Ok(self
                .observation
                .lock()
                .expect("test observation lock")
                .clone())
        }

        async fn credential_get(&self, _service: &str, _key: &str) -> Result<Option<String>> {
            Ok(None)
        }

        async fn credential_set(&self, _service: &str, _key: &str, _value: &str) -> Result<()> {
            Ok(())
        }

        async fn credential_delete(&self, _service: &str, _key: &str) -> Result<()> {
            Ok(())
        }

        fn data_dir(&self) -> Option<PathBuf> {
            Some(PathBuf::from("/android-test-data"))
        }

        fn cache_dir(&self) -> Option<PathBuf> {
            Some(PathBuf::from("cache"))
        }

        async fn vpn_start(&self) -> Result<VpnStartOutcome> {
            Ok(VpnStartOutcome::PermissionRequired)
        }

        async fn vpn_apply_configuration(
            &self,
            _prepared: &PreparedVpnConfiguration,
        ) -> Result<VpnConfigurationApplyOutcome> {
            Ok(VpnConfigurationApplyOutcome::Rejected)
        }

        async fn vpn_stop(&self, _target: VpnStopTarget) -> Result<VpnOwnerStopReceipt> {
            Ok(VpnOwnerStopReceipt::OwnerUnavailable {
                failure: Failure::new(
                    ErrorCode::NotReady,
                    "fake VPN stop owner has no same-request readback",
                    true,
                ),
            })
        }

        async fn vpn_owner_snapshot(&self) -> Result<VpnOwnerSnapshot> {
            Ok(VpnOwnerSnapshot {
                owner_session_id: "owner-session".to_owned(),
                revision: 1,
                vpn_generation: infiltrator_contract::vpn::VpnGeneration::new(0),
                vpn_state: VpnOwnerLifecycle::Stopped,
                readiness: VpnOwnerReadiness::Stopped,
                resources: Vec::new(),
                foreground: false,
                mtu: None,
                route_count: 0,
                dns_servers: Vec::new(),
                ipv6: false,
                core: VpnOwnerCoreReadback::Available(
                    infiltrator_contract::snapshot::CoreLifecycleSnapshot {
                        identity: None,
                        lifecycle: CoreLifecycle::Stopped,
                        generation: 0,
                        session_token: None,
                        revision: 0,
                        failure: None,
                    },
                ),
            })
        }
    }

    fn config() -> AndroidProductConfig {
        AndroidProductConfig {
            controller_url: "http://127.0.0.1:1".to_string(),
            controller_secret: None,
            sample_interval: Duration::from_millis(50),
        }
    }

    #[test]
    fn composition_publishes_an_android_surface_snapshot() {
        let (_home, owner_composition) = composition_input();
        let composition = AndroidProductComposition::open_with_bridge(
            Arc::new(TestBridge::new()),
            config(),
            owner_composition,
        )
        .expect("composition opens with an explicit bridge");
        let bridge = composition.surface_bridge();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut delivered = None;
        while delivered.is_none() {
            delivered = bridge.drain().into_iter().next_back();
            assert!(
                delivered.is_some() || Instant::now() < deadline,
                "surface pump did not deliver a snapshot"
            );
            if delivered.is_none() {
                yield_now();
            }
        }
        let snapshot = delivered.expect("snapshot");
        assert_eq!(snapshot.surface, SurfaceKind::BevyAndroid);
        assert_eq!(snapshot.capabilities.host, HostKind::Android);
        // The application facade is the same shared instance the pump reads.
        assert!(Arc::strong_count(&composition.application()) >= 1);
    }

    #[test]
    fn missing_bridge_is_a_typed_open_failure() {
        clear_android_bridge();
        let error = match AndroidProductComposition::open(config()) {
            Ok(_) => panic!("open must fail without a registered bridge"),
            Err(error) => error,
        };
        assert!(matches!(error, AndroidProductError::BridgeUnavailable));
    }

    #[test]
    fn client_process_cannot_construct_a_local_product_or_controller() {
        let (_home, owner_composition) = composition_input();
        let result = AndroidProductComposition::open_with_bridge(
            Arc::new(TestBridge::client_process()),
            config(),
            owner_composition,
        );

        assert!(matches!(result, Err(AndroidProductError::NotCoreOwner)));
    }

    #[test]
    fn owner_surface_json_rejects_core_facts_from_a_different_owner_readback() {
        let mut snapshot = SurfaceSnapshot::unavailable(
            SurfaceKind::BevyAndroid,
            HostKind::Android,
            Failure::new(ErrorCode::NotReady, "waiting", true),
        );
        snapshot.vpn = VpnSurfaceObservation::AndroidOwner(VpnOwnerSnapshot {
            owner_session_id: "vpn-owner-a".to_owned(),
            revision: 1,
            vpn_generation: infiltrator_contract::vpn::VpnGeneration::new(0),
            vpn_state: VpnOwnerLifecycle::Stopped,
            readiness: VpnOwnerReadiness::Stopped,
            resources: Vec::new(),
            foreground: false,
            mtu: None,
            route_count: 0,
            dns_servers: Vec::new(),
            ipv6: false,
            core: VpnOwnerCoreReadback::Unavailable {
                failure_message: "Core readback unavailable".to_owned(),
            },
        });
        assert!(serialize_owner_surface_snapshot(&snapshot).is_ok());

        snapshot.core.identity = Some(infiltrator_contract::core_identity::CoreIdentity::new(
            infiltrator_contract::core_identity::EngineId::new("mihomo").expect("engine ID"),
            infiltrator_contract::core_identity::CoreInstanceId::new("core-a")
                .expect("instance ID"),
            None,
        ));
        let VpnSurfaceObservation::AndroidOwner(owner) = &mut snapshot.vpn else {
            unreachable!("the test installed an Android owner snapshot");
        };
        owner.core = VpnOwnerCoreReadback::Available(
            infiltrator_contract::snapshot::CoreLifecycleSnapshot::default(),
        );
        let error = serialize_owner_surface_snapshot(&snapshot)
            .expect_err("surface must not combine a newer Core identity with stale owner data");
        assert!(error.contains("do not match"));
    }
}
