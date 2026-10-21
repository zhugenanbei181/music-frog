# infiltrator-test-support

> 层级：L3

This crate owns engine-neutral fake ports, in-memory stores, and deterministic
documents shared by Core tests and isolated UI captures. It depends only on
contract, domain, and ports. It does not create product runtimes or select an
engine adapter; product compositions inject concrete codecs separately.

Application uses the crate only as a dev-dependency. `infiltrator-composition`
enables it behind `capture-fixtures`; Iced and Bevy capture tools consume the
same fixture definitions without reaching into Application's test modules.

## Fixture index

Every fixture answers the owner's question with its real published snapshot,
refuses a request it cannot honour with a typed failure, and rejects a stale
revision instead of overwriting a newer state. The behaviour tests live in
`tests/port_fixture_tests.rs` (one module per fixture).

| Port | Fixture module | Publishes | Unblocks |
| --- | --- | --- | --- |
| `VersionPort` | `version_fixtures` (`VersionCapturePort`) | installed versions, release channels, release list, local rollback journal, install verification | `core-version-rollback-confirm` |
| `PortConflictPort` | `port_conflict_fixtures` (`PortConflictCapturePort`) | taken/free bindings with their owners and release rights | `settings-port-conflict-repair` |
| `ServiceModePort` | `service_mode_fixtures` (`ServiceModeCapturePort`) | platform plus privilege state (`Ready`, `InstalledStopped`, `MissingPrivilege`, `Unsupported`) | `settings-tun-permission` |
| `LanConfigurationPort` | `lan_configuration_fixtures` (`LanConfigurationCapturePort`) | LAN sharing and LAN security observations with a configuration revision | `settings-lan-security` |
| `PrivilegedNetworkPort` | `privileged_network_fixtures` (`PrivilegedNetworkCapturePort`) | injection/cleanup transaction state and rollback evidence | `settings-permission-panel` |
| `ProfileStore` | `profile_store_fixtures` (`ProfileStoreCapture`) | small profile, large profile document, metadata, aggregation-template library, active-profile reference | `profiles-delete-confirm`, `profiles-aggregator-preview`, `editor-large-document-window` |
| `SubscriptionSource` | `subscription_source_fixtures` (`SubscriptionSourceCapture`) | a prepared document, or a real network failure when the source refuses | `profiles-subscription-error` |
| `SubscriptionImportPort` | `subscription_import_fixtures` (`SubscriptionImportCapture`) | one local file and one clipboard entry | `profiles-import-modal` |
| `RuleProviderPort` | `rule_provider_fixtures` (`RuleProviderCapturePort`) | provider inventory, refresh receipt, payload readback | `rules-provider-refresh-error` |
| `RuleProviderCachePort` | `rule_provider_fixtures` (`RuleProviderCacheCapture`) | cached provider bytes, file fingerprints, honest purge counts | `rules-provider-refresh-error` |
| `SnapshotStore` | `snapshot_store_fixtures` (`SnapshotStoreCapture`) | snapshot metadata with the real content digest and history path | `editor-yaml-error-navigation` |
| `SpeedtestHistoryStore` | `speedtest_history_fixtures` (`SpeedtestHistoryCapture`) | two comparable runs of one target; `running_speedtest_snapshot()` publishes a run that is still probing | `speedtest-history-comparison`, `speedtest-progress-cancel` |
| `ProfileDocumentCodecPort` | `profile_document_codec_fixtures` (`ProfileDocumentCodecCapture`) | real Mihomo YAML validation and the shared snippet splice | `editor-snippet-insertion` |
| `CoreProcess` / `CoreReadiness` | `core_process_fixtures` (`CoreCaptureProcess`) | lifecycle, readiness probe, controller endpoint, process id | `overview-reconnect-mask`, `speedtest-progress-cancel` |

### Honest refusals

- `SubscriptionSourceCapture::refusing` really fails the fetch; the capture
  never sees a fabricated document.
- `SubscriptionImportCapture::without_clipboard` answers `read_clipboard` with
  a typed `Unsupported` instead of an empty document.
- `RuleProviderCacheCapture` answers `Ok(None)` for a provider it never
  downloaded and reports real purge counts.
- `ProfileStoreCapture` answers `NotFound` for a profile it does not hold,
  answers `Unsupported` for the capabilities it does not have (no selected
  active profile, no aggregation sidecar), and refuses to delete or overwrite
  the active profile.
- `CoreCaptureProcess::with_behaviour(CoreCaptureBehaviour::StalledOnStart)`
  keeps the core in `Starting`; the readiness probe fails with a typed
  `NotReady` failure.

