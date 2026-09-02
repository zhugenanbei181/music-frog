use super::*;

#[test]
fn muda_id_round_trips_action_and_payload() {
    assert_eq!(muda_id_for(7, None), "tray-action-7");
    let TrayEvent::MenuActivated { id, payload } = translate_menu_id("tray-action-7") else {
        panic!("must parse into a menu activation");
    };
    assert_eq!((id, payload.as_deref()), (7, None));

    let payload = "GLOBAL\u{1}node:1".to_string();
    let wire = muda_id_for(31, Some(&payload));
    assert_eq!(wire, format!("tray-action-31:{payload}"));
    let TrayEvent::MenuActivated { id, payload: back } = translate_menu_id(&wire) else {
        panic!("must parse into a menu activation");
    };
    assert_eq!(id, 31);
    assert_eq!(back.as_deref(), Some(payload.as_str()));
}

#[test]
fn translate_menu_id_rejects_foreign_ids_via_placeholder() {
    let TrayEvent::MenuActivated { id, payload } = translate_menu_id("not-ours") else {
        panic!("fallback must still be a menu activation");
    };
    assert_eq!(id, TRAY_ACTION_NO_PROXIES);
    assert!(payload.is_none());

    let TrayEvent::MenuActivated { id, .. } = translate_menu_id("tray-action-99999:big") else {
        panic!("fallback must still be a menu activation");
    };
    assert_eq!(id, TRAY_ACTION_NO_PROXIES);
}

fn entry(muda_id: &str, key: CheckedOverrideKey, checked: bool) -> (String, CheckEntry) {
    (muda_id.to_string(), CheckEntry { key, checked })
}

#[test]
fn flip_toggles_display_state_and_records_override() {
    let mut registry = HashMap::from([entry("tray-action-7", (7, None), false)]);
    let mut overrides = HashMap::new();

    assert_eq!(
        flip_checkmark_in(&mut registry, &mut overrides, "tray-action-7"),
        Some(((7, None), true))
    );
    assert_eq!(overrides.get(&(7, None)), Some(&true));
    assert!(registry["tray-action-7"].checked);

    assert_eq!(
        flip_checkmark_in(&mut registry, &mut overrides, "tray-action-7"),
        Some(((7, None), false))
    );
    assert_eq!(overrides.get(&(7, None)), Some(&false));
}

#[test]
fn flip_resolves_payload_scoped_keys_without_crosstalk() {
    let payload_a = "AUTO_UPDATE\u{1}a".to_string();
    let payload_b = "AUTO_UPDATE\u{1}b".to_string();
    let mut registry = HashMap::from([
        entry(
            &format!("tray-action-9:{payload_a}"),
            (9, Some(payload_a.clone())),
            false,
        ),
        entry(
            &format!("tray-action-9:{payload_b}"),
            (9, Some(payload_b.clone())),
            true,
        ),
    ]);
    let mut overrides = HashMap::new();

    assert_eq!(
        flip_checkmark_in(
            &mut registry,
            &mut overrides,
            &format!("tray-action-9:{payload_a}")
        ),
        Some(((9, Some(payload_a.clone())), true))
    );
    assert_eq!(
        flip_checkmark_in(
            &mut registry,
            &mut overrides,
            &format!("tray-action-9:{payload_b}")
        ),
        Some(((9, Some(payload_b.clone())), false))
    );
    assert_eq!(overrides.len(), 2);
}

#[test]
fn flip_ignores_foreign_and_stale_ids_without_mutation() {
    let mut registry = HashMap::from([entry("tray-action-3", (3, None), true)]);
    let mut overrides = HashMap::new();

    assert_eq!(
        flip_checkmark_in(&mut registry, &mut overrides, "not-ours"),
        None
    );
    assert_eq!(
        flip_checkmark_in(&mut registry, &mut overrides, "tray-action-99999:x"),
        None
    );
    assert!(overrides.is_empty());
    assert!(registry["tray-action-3"].checked);
}

#[test]
fn clear_overrides_semantics_match_ksni_spec_push() {
    let mut overrides: HashMap<CheckedOverrideKey, bool> = HashMap::from([((4, None), true)]);
    let mut registry = HashMap::from([entry("tray-action-4", (4, None), false)]);

    assert_eq!(
        flip_checkmark_in(&mut registry, &mut overrides, "tray-action-4"),
        Some(((4, None), false))
    );

    overrides.clear();
    registry.get_mut("tray-action-4").unwrap().checked = false;
    assert_eq!(
        flip_checkmark_in(&mut registry, &mut overrides, "tray-action-4"),
        Some(((4, None), true))
    );
}
