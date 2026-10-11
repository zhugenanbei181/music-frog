"""Positive and negative contract tests for the typed-state source guard."""

import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest

QUALITY = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "typed_state_guard",
    QUALITY / "typed-state-guard.py",
)
GUARD = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = GUARD
SPEC.loader.exec_module(GUARD)


def codes(path: str, source: str) -> set[str]:
    return {problem.code for problem in GUARD.analyze_source(Path(path), source)}


class TypedStateGuardTests(unittest.TestCase):
    def test_incremental_scope_filters_findings_without_changing_full_scan(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            selected = root / "crates/core/src/state.rs"
            unrelated = root / "crates/ui/src/state.rs"
            selected.parent.mkdir(parents=True)
            unrelated.parent.mkdir(parents=True)
            selected.write_text("struct CoreState;", encoding="utf-8")
            unrelated.write_text("struct UiState;", encoding="utf-8")
            core_problem = GUARD.Problem(1, "CORE", "core state")
            ui_problem = GUARD.Problem(1, "UI", "ui state")
            findings = [(selected, core_problem), (unrelated, ui_problem)]

            self.assertEqual(
                GUARD.filter_findings(findings, [Path("crates/core/src/state.rs")], root),
                [(selected, core_problem)],
            )
            self.assertEqual(GUARD.filter_findings(findings, [], root), findings)
            with self.assertRaisesRegex(ValueError, "escapes repository"):
                GUARD.filter_findings(findings, [Path("../outside.rs")], root)

    def test_enum_states_and_independent_binary_facts_are_accepted(self):
        self.assertEqual(codes("src/vpn_lifecycle_repository.rs", """
            enum LifecycleState { Stopped, Starting, Running, Failed }
            struct VpnSettingsState { enabled: bool, ipv6: bool }
            struct VpnSettingsRepositoryState { enabled: bool, config: Option<String> }
            trait VpnLifecycleRepository {
                fn set_enabled(&self, enabled: bool);
                fn is_enabled(&self) -> bool;
            }
        """), set())

    def test_kotlin_enum_projection_is_accepted(self):
        self.assertEqual(codes("android/src/main/kotlin/CoreLifecycleViewModel.kt", """
            class CoreLifecycleViewModel {
                sealed interface State {
                    data object Loading : State
                    data class Ready(val revision: Long) : State
                }
                fun start(): State = State.Ready(1)
                fun isRunning(): Boolean = true
            }
        """), set())

    def test_independent_page_errors_and_computed_sealed_state_projections_are_accepted(self):
        self.assertEqual(codes("android/app/src/main/java/OverviewViewModel.kt", """
            data class OverviewUiState(
                val coreLifecycleError: LocalizedFailure?,
                val vpnStateError: LocalizedFailure?,
                val error: LocalizedFailure?,
            )
            sealed interface TunOperationState {
                data object Loading : TunOperationState
                data class Failed(val failure: AppFailure) : TunOperationState
            }
            data class TunUiState(val operation: TunOperationState) {
                val isBusy: Boolean
                    get() = operation is TunOperationState.Loading
                val failure: AppFailure?
                    get() = (operation as? TunOperationState.Failed)?.failure
            }
        """), set())

    def test_scalar_settings_and_local_change_facts_are_accepted(self):
        self.assertEqual(codes("crates/infiltrator-application/src/runtime_query_application.rs", """
            struct RuntimeQueryApplication;
            impl RuntimeQueryApplication {
                async fn set_tun_auto_route(&self, enabled: bool) -> Result<(), Failure> { todo!() }
                async fn set_tun_routing(&self, auto_route: bool, strict_route: bool) -> Result<(), Failure> { todo!() }
                async fn update_sync_enabled(&self, value: bool) -> Result<(), Failure> { todo!() }
            }
        """), set())
        self.assertEqual(codes("crates/infiltrator-ports/src/fake_ip_cache.rs", """
            trait FakeIpCachePort {
                async fn clear(&self) -> Result<bool, PortError>;
                async fn remove_expired(&self) -> Result<bool, PortError>;
                async fn consume_next(&self) -> Result<bool, PortError>;
                async fn update_search(&self, query: String) -> Result<bool, PortError>;
            }
        """), set())
        self.assertEqual(codes("crates/infiltrator-application/src/profile_aggregation_application.rs", """
            struct ProfileAggregationApplication;
            impl ProfileAggregationApplication {
                async fn delete_template(&self, name: &str) -> Result<bool, Failure> { todo!() }
            }
        """), set())
        self.assertEqual(codes("crates/infiltrator-bevy-widgets/src/text_input/state.rs", """
            struct TextFieldState { is_disabled: bool }
            impl TextFieldState {
                fn apply(&mut self, input: Input) -> bool {
                    if self.is_disabled { return false; }
                    self.apply_input(input)
                }
            }
        """), set())

    def test_enum_branch_and_framework_handled_result_are_not_flag_transitions(self):
        self.assertEqual(codes("android/app/src/main/java/OwnerIpcClient.kt", """
            enum class VpnState { Starting, Running, Stopped }
            class OwnerIpcClient(private val vpnState: VpnState?) {
                fun readResponse() {
                    when (vpnState) {
                        VpnState.Starting, VpnState.Running, VpnState.Stopped, null -> Unit
                    }
                }
            }
        """), set())
        self.assertEqual(codes("android/app/src/main/java/MihomoControlService.kt", """
            class MihomoControlService {
                private fun handleMessage(message: Message): Boolean = true
            }
        """), set())

    def test_inline_cfg_test_module_is_not_scanned_as_production(self):
        self.assertEqual(codes("crates/infiltrator-application/src/cache_application.rs", """
            struct CacheApplication;
            impl CacheApplication { fn clear(&self) -> Result<bool, Failure> { todo!() } }
            #[cfg(test)]
            mod tests {
                struct FakeRepositoryState { is_loading: bool, is_running: bool }
                trait OwnerRepository { fn start(&self, force: bool) -> bool; }
            }
        """), set())

    def test_boundary_state_records_reject_exclusive_flag_fields(self):
        self.assertIn("MULTI_FIELD_STATE", codes("src/vpn/service_state.rs", """
            struct VpnServiceState {
                is_loading: bool,
                is_running: bool,
                error: Option<String>,
            }
        """))
        self.assertIn("MULTI_FIELD_STATE", codes("android/src/main/kotlin/ProxyViewModel.kt", """
            data class ProxyViewModelState(
                val isLoading: Boolean,
                val error: String?,
                val saved: Boolean,
            )
        """))

    def test_operation_flags_cannot_be_combined_with_cancel_or_progress_options(self):
        findings = codes("crates/infiltrator-iced/src/state/profiles.rs", """
            struct ProfileState {
                is_syncing: bool,
                sync_cancel: Option<Arc<AtomicBool>>,
                sync_progress: Option<SyncProgress>,
                is_testing_webdav: bool,
            }
        """)
        self.assertIn("MULTI_FIELD_STATE", findings)
        self.assertIn("UNTYPED_OPERATION_STATE", findings)
        self.assertEqual(codes("crates/infiltrator-iced/src/state/webdav_sync.rs", """
            enum SyncPhase { Idle, Uploading, Downloading, Failed(Failure) }
            struct SyncState { phase: SyncPhase, progress: Option<SyncProgress> }
        """), set())

    def test_operation_boolean_parameters_and_wrapped_results_are_rejected(self):
        self.assertIn("BOOL_OPERATION_PARAMETER", codes("src/vpn/lifecycle_repository.rs", """
            trait VpnLifecycleRepository {
                fn start<'a>(&self, force: std::option::Option<bool>);
            }
        """))
        self.assertIn("BOOL_OPERATION_RESULT", codes("android/src/main/kotlin/OwnerIpcClient.kt", """
            class OwnerIpcClient {
                fun <T> attach(): Result<Boolean, T> = TODO()
            }
        """))
        self.assertIn("BOOL_OPERATION_PARAMETER", codes("crates/infiltrator-application/src/script_application.rs", """
            struct ScriptApplication;
            impl ScriptApplication {
                fn run_observed(&self, validate_result: bool) -> Result<Receipt, Failure> { todo!() }
            }
        """))
        self.assertIn("BOOL_OPERATION_RESULT", codes("crates/infiltrator-application/src/profile_aggregation_application.rs", """
            struct ProfileAggregationApplication;
            impl ProfileAggregationApplication {
                async fn activate_generated_profile(&self) -> Result<(bool, bool), Failure> { todo!() }
            }
        """))
        self.assertIn("BOOL_OPERATION_RESULT", codes("android/app/src/main/java/MihomoHost.kt", """
            class MihomoHost {
                fun coreStart(): Boolean = true
                fun coreStop(): Boolean = false
            }
        """))
        self.assertIn("BOOL_OPERATION_RESULT", codes("android/app/src/main/java/VpnServiceBindingState.kt", """
            class VpnServiceBindingState {
                fun beginRebindAfterDeath(): Boolean = true
                fun bindingDied(): Boolean {
                    update(State.Recovering)
                    return true
                }
                private fun update(next: State) {}
            }
        """))

    def test_flag_based_transition_is_rejected_but_unrelated_local_logic_is_not(self):
        self.assertIn("FLAG_TRANSITION", codes("android/src/main/kotlin/VpnLifecycleService.kt", """
            class VpnLifecycleService {
                var isRunning: Boolean = false
                fun stop() {
                    if (isRunning) {
                        isRunning = false
                        release()
                    }
                }
                fun release() {}
            }
        """))
        self.assertEqual(codes("src/vpn/lifecycle_repository.rs", """
            trait VpnLifecycleRepository { fn start(&self); }
            fn format_label(enabled: bool) -> String { enabled.to_string() }
        """), set())
        self.assertIn("FLAG_TRANSITION", codes("src/vpn/lifecycle_repository.rs", """
            struct VpnService { is_running: bool }
            impl VpnLifecycleRepository for VpnService {
                fn stop(&mut self) {
                    if self.is_running { self.is_running = false; }
                }
            }
        """))

    def test_lexical_masks_and_following_declaration_do_not_change_scope(self):
        source = '''
            // data class State(val isLoading: Boolean, val isRunning: Boolean)
            class VpnLifecycleService(val isLoading: Boolean)
            fun unrelated() { val label = "if (isLoading) { start() }" }
            val raw = """false then stop()"""
            fun isReady(): Boolean = true
        '''
        self.assertEqual(codes("android/src/main/kotlin/VpnLifecycleService.kt", source), set())

    def test_annotations_cannot_suppress_boundary_violations(self):
        found = codes("android/src/main/kotlin/OwnerLifecycleRepository.kt", """
            @Suppress("TypedState")
            class OwnerLifecycleRepository {
                fun pause(force: Boolean) {}
            }
        """)
        self.assertIn("BOOL_OPERATION_PARAMETER", found)


if __name__ == "__main__":
    unittest.main()
