use super::*;

#[test]
#[ignore = "requires UniFFI Kotlin codegen output; run with --run-ignored after Android codegen"]
fn current_host_protocol_and_owner_operations_match_generated_kotlin() {
    let control = parse_rust(&repository_file(CONTROL));
    let owner = parse_rust(&repository_file(OWNER));
    let generated = repository_file(BINDINGS);
    for (file, type_name) in [
        (&control, "AndroidHostProtocolVersion"),
        (&control, "AndroidHostRequest"),
        (&control, "AndroidHostReply"),
    ] {
        let rust = enum_item(file, type_name).expect("typed host contract enum must exist");
        let expected = enum_variants(rust)
            .iter()
            .map(|variant| snake_case(variant))
            .collect::<BTreeSet<_>>();
        let actual = kotlin_enum_variants(&generated, type_name)
            .unwrap_or_else(|| panic!("generated Kotlin enum {type_name} is missing"));
        assert_eq!(
            actual, expected,
            "generated Kotlin enum {type_name} drifted from Rust"
        );
    }

    let rust_owner = enum_item(&owner, "AndroidOwnerOperation")
        .expect("typed Android owner operation enum must exist");
    let expected_owner = enum_variants(rust_owner);
    let actual_owner = kotlin_sealed_variants(&generated, "AndroidOwnerOperation")
        .expect("generated Kotlin AndroidOwnerOperation must remain sealed");
    assert_eq!(
        actual_owner, expected_owner,
        "generated Kotlin owner variants drifted from Rust"
    );
}

#[test]
fn generated_binding_variant_parser_accepts_match_and_rejects_drift_fixtures() {
    let rust = parse_rust("enum AndroidHostRequest { ReadLifecycle }");
    let expected = enum_variants(enum_item(&rust, "AndroidHostRequest").unwrap())
        .iter()
        .map(|variant| snake_case(variant))
        .collect::<BTreeSet<_>>();
    let matching = "enum class AndroidHostRequest { READ_LIFECYCLE }";
    assert_eq!(
        kotlin_enum_variants(matching, "AndroidHostRequest"),
        Some(expected.clone())
    );
    let stale = "enum class AndroidHostRequest { READ_LIFECYCLE, LOCAL_START }";
    assert_ne!(
        kotlin_enum_variants(stale, "AndroidHostRequest"),
        Some(expected)
    );

    let owner = parse_rust("enum AndroidOwnerOperation { ProfilesList, NewOperation }");
    let expected_owner = enum_variants(enum_item(&owner, "AndroidOwnerOperation").unwrap());
    let generated_owner = "sealed class AndroidOwnerOperation {\n    object ProfilesList : AndroidOwnerOperation()\n    data class NewOperation(val value: String) : AndroidOwnerOperation()\n}";
    assert_eq!(
        kotlin_sealed_variants(generated_owner, "AndroidOwnerOperation"),
        Some(expected_owner.clone())
    );
    let stale_owner = "sealed class AndroidOwnerOperation {\n    object ProfilesList : AndroidOwnerOperation()\n}";
    assert_ne!(
        kotlin_sealed_variants(stale_owner, "AndroidOwnerOperation"),
        Some(expected_owner)
    );
}

#[test]
fn every_owner_operation_is_exhaustively_registered_and_dispatched() {
    assert_eq!(
        operation_coverage_problems(&repository_file(OWNER), &repository_file(OWNER_WIRE)),
        Vec::<String>::new(),
    );
}

#[test]
fn owner_operation_guard_rejects_an_unregistered_variant_and_wildcard_fallback() {
    let operation = "enum AndroidOwnerOperation { Existing, Added } impl AndroidOwnerOperation { fn data_domain(&self) { match self { Self::Existing => (), } } fn data_mutation(&self) { match self { Self::Existing => (), Self::Added => (), } } }";
    let wire = "fn execute_operation(operation: AndroidOwnerOperation) { match operation { AndroidOwnerOperation::Existing => (), AndroidOwnerOperation::Added => (), } }";
    let expected = operation_coverage_problems(operation, wire);
    assert!(
        expected
            .iter()
            .any(|problem| problem.contains("data_domain"))
    );

    let fully_registered = "enum AndroidOwnerOperation { Existing, Added } impl AndroidOwnerOperation { fn data_domain(&self) { match self { Self::Existing => (), Self::Added => (), } } fn data_mutation(&self) { match self { Self::Existing => (), Self::Added => (), } } }";
    let wildcard_wire = "fn execute_operation(operation: AndroidOwnerOperation) { match operation { AndroidOwnerOperation::Existing => (), _ => (), } }";
    let expected = operation_coverage_problems(fully_registered, wildcard_wire);
    assert!(
        expected
            .iter()
            .any(|problem| problem.contains("wildcard fallback"))
    );
}

#[test]
fn lifecycle_exports_reject_boolean_results_and_string_actions() {
    let typed = "#[uniffi::export] pub fn android_control(intent: AndroidControlIntent) -> FfiStatus { todo!() } #[uniffi::export] pub fn prepare_vpn(generation: u64) -> VpnPreparationResult { todo!() }";
    assert_eq!(exported_contract_problems(typed), Vec::<String>::new());

    let boolean = "#[uniffi::export] pub fn vpn_is_running() -> bool { true }";
    assert!(
        exported_contract_problems(boolean)
            .iter()
            .any(|problem| problem.contains("typed outcome"))
    );

    let boolean_action =
        "#[uniffi::export] pub fn set_vpn_running(running: bool) -> FfiStatus { todo!() }";
    assert!(
        exported_contract_problems(boolean_action)
            .iter()
            .any(|problem| problem.contains("boolean"))
    );

    let string_action =
        "#[uniffi::export] pub fn android_control(action: String) -> FfiStatus { todo!() }";
    assert!(
        exported_contract_problems(string_action)
            .iter()
            .any(|problem| problem.contains("typed enum"))
    );
}

#[test]
fn production_uniffi_exports_keep_lifecycle_and_action_parameters_typed() {
    let problems = collect_exported_functions(Path::new(CRATE_ROOT))
        .into_iter()
        .flat_map(|(path, function)| {
            exported_function_problems(&function)
                .into_iter()
                .map(move |problem| format!("{path}: {problem}"))
        })
        .collect::<Vec<_>>();
    assert_eq!(problems, Vec::<String>::new());
}

#[test]
fn udl_rejects_boolean_lifecycle_exports_and_string_actions() {
    let current = repository_file(UDL);
    assert_eq!(udl_contract_problems(&current), Vec::<String>::new());
    assert_eq!(
        udl_contract_problems("FfiStatus android_control(AndroidControlIntent intent);"),
        Vec::<String>::new()
    );
    assert!(
        udl_contract_problems("boolean vpn_is_running();")
            .iter()
            .any(|problem| problem.contains("typed outcome"))
    );
    assert!(
        udl_contract_problems("FfiStatus android_control(string action);")
            .iter()
            .any(|problem| problem.contains("typed enum"))
    );
    assert!(
        udl_contract_problems("FfiStatus set_vpn_running(boolean running);")
            .iter()
            .any(|problem| problem.contains("boolean"))
    );
}

#[test]
#[ignore = "requires UniFFI Kotlin codegen output; run with --run-ignored after Android codegen"]
fn generated_binding_keeps_lifecycle_and_command_entrypoints_typed() {
    let generated = repository_file(BINDINGS);
    let expected = [
        ("startVpn", "FfiStatus"),
        ("prepareVpn", "VpnPreparationResult"),
        ("cancelVpnPreparation", "FfiStatus"),
        ("stopVpnWorker", "FfiStatus"),
        ("revokeVpn", "FfiStatus"),
        (
            "androidBeginVpnConfigurationActivation",
            "AndroidVpnConfigurationActivationResult",
        ),
        (
            "androidFinishVpnConfigurationActivation",
            "AndroidVpnConfigurationActivationReleaseResult",
        ),
    ];
    for (name, result_type) in expected {
        let signature = kotlin_function_signature(&generated, name)
            .unwrap_or_else(|| panic!("generated Kotlin FFI function {name} is missing"));
        assert!(
            signature.contains(&format!(": {result_type}")),
            "{name} lost its typed result: {signature}"
        );
        assert!(
            !signature.contains("kotlin.Boolean") && !signature.contains("kotlin.String"),
            "{name} has an untyped lifecycle field: {signature}"
        );
    }
    let control = kotlin_function_signature(&generated, "androidControl")
        .expect("generated typed control function must exist");
    assert!(
        control.contains("intent") && control.contains("AndroidControlIntent"),
        "control argument became untyped: {control}"
    );
    let owner = kotlin_function_signature(&generated, "androidExecuteOwnerOperation")
        .expect("generated typed owner operation function must exist");
    assert!(
        owner.contains("operation") && owner.contains("AndroidOwnerOperation"),
        "owner argument became untyped: {owner}"
    );
}

#[test]
fn ui_sources_cannot_shadow_the_typed_owner_contract() {
    let root = Path::new(CRATE_ROOT);
    let ui_sources = kotlin_ui_declarations(root);
    let borrowed = ui_sources
        .iter()
        .map(|(path, source)| (path.as_str(), source.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(ui_fallback_problems(&borrowed), Vec::<String>::new());

    let fixture = [("FakeUi.kt", "sealed interface AndroidOwnerOperation")];
    assert!(
        ui_fallback_problems(&fixture)
            .iter()
            .any(|problem| problem.contains("local AndroidOwnerOperation fallback"))
    );
}
