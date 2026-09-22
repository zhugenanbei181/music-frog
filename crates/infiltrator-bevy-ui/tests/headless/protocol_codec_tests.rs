//! DUAL-05 headless integration tests: the Bevy custom-node card is a
//! projection of the shared `ProtocolStudioSnapshot`.
//!
//! Proves that the import button submits the typed URI intent, that the save
//! button submits the *shared* draft (and refuses when the application has
//! published none), and that cipher family (05-01), REALITY/Vision (05-02),
//! multiplexing (05-11) and the codec audit (05-14) all render from that
//! single shared source.

use std::sync::Arc;

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ui_widgets::Activate;
use infiltrator_application::protocol_codec_application::{ProtocolCodecApplication, clear_studio};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::pages::proxies::{ProxiesProjection, ProxiesProjectionUpdated};
use infiltrator_bevy_ui::pages::proxies_custom::{
    CustomNodeCaField, CustomNodeDialerField, CustomNodeSlot, CustomNodeText, CustomNodeUriField,
    ImportUriButton, SaveCustomNodeButton, ScanDialerChainsButton, VerifyCustomNodeCaButton,
};
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::route::{PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_contract::protocol_fidelity::ProtocolDraft;

use crate::support::*;

/// 32-byte base64 key used by the WireGuard fixture.
const WG_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

const VLESS_URI: &str = "vless://b831381d-6324-4d53-ad4f-8cda48b30811@us.example.com:443?security=reality&pbk=PubKey1234567890AAAAAAAAAAAAAAAAAAAAAAAAAAAAA&sid=abcd1234&spx=%2Fspider&fp=chrome&flow=xtls-rprx-vision&sni=reality.example.com#US-Reality";

fn setup_app(sink: Arc<DemoCommandSink>) -> App {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new(DemoOverviewSource::running()));
    app.add_plugins(CommandPumpPlugin::new(sink as Arc<dyn UiCommandSink>));
    app.update();
    app
}

fn navigate_to_proxies(app: &mut App) -> Entity {
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();
    let (root, route) = page_root(app.world_mut());
    assert_eq!(route, Route::Proxies);
    root
}

fn button_entity<T: bevy::ecs::component::Component>(app: &mut App) -> Entity {
    let mut query = app.world_mut().query::<(Entity, &T)>();
    query
        .iter(app.world())
        .next()
        .map(|(entity, _)| entity)
        .expect("marker component entity")
}

fn text_for_slot(app: &mut App, slot: CustomNodeSlot) -> String {
    let mut query = app
        .world_mut()
        .query::<(&bevy::ui::widget::Text, &CustomNodeText)>();
    let mut found = None;
    for (text, marker) in query.iter(app.world()) {
        if marker.0 == slot {
            found = Some(text.0.clone());
        }
    }
    found.expect("custom node text slot")
}

#[test]
fn custom_node_import_button_submits_the_typed_uri() {
    clear_studio();
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_app(Arc::clone(&sink));
    navigate_to_proxies(&mut app);

    let field = {
        let mut fields = app.world_mut().query::<(&CustomNodeUriField, &Children)>();
        *fields
            .single(app.world())
            .expect("uri field wrapper")
            .1
            .iter()
            .next()
            .expect("uri text field")
    };
    app.world_mut()
        .get_mut::<TextField>(field)
        .expect("uri field state")
        .0
        .apply(TextFieldInput::SetText(VLESS_URI.to_owned()));

    let import = button_entity::<ImportUriButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: import });
    app.update();

    assert_eq!(
        sink.submitted(),
        vec![UiCommand::ImportCustomNodeUri {
            uri: VLESS_URI.to_owned(),
        }]
    );

    // An empty field submits nothing: the UI never fabricates a URI.
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_app(Arc::clone(&sink));
    navigate_to_proxies(&mut app);
    let import = button_entity::<ImportUriButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: import });
    app.update();
    assert!(sink.submitted().is_empty());
}

#[test]
fn custom_node_save_refuses_without_a_shared_draft_and_submits_the_shared_one() {
    clear_studio();
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_app(Arc::clone(&sink));
    navigate_to_proxies(&mut app);

    let save = button_entity::<SaveCustomNodeButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();
    assert!(
        sink.submitted().is_empty(),
        "no shared draft means no save command"
    );

    // Publish a real draft through the shared application and re-project.
    let draft = ProtocolCodecApplication::draft_from_uri(VLESS_URI).unwrap();
    let preview = ProtocolCodecApplication::uri_from_draft(&draft).ok();
    let studio = ProtocolCodecApplication::publish_draft(draft.clone(), preview);
    let projection = ProxiesProjection {
        groups: Vec::new(),
        testing: false,
        active_exit: "—".to_owned(),
        custom_node: studio,
    };
    let save = button_entity::<SaveCustomNodeButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(projection));
    app.update();
    app.world_mut()
        .commands()
        .trigger(Activate { entity: save });
    app.update();

    match sink.submitted().last() {
        Some(UiCommand::SaveCustomNodeDraft { draft: submitted }) => {
            assert_eq!(submitted.as_ref(), &draft);
            assert_eq!(submitted.name, "US-Reality");
            assert_eq!(submitted.flow, "xtls-rprx-vision");
        }
        other => panic!("expected the shared draft to be submitted, got {other:?}"),
    }
}

#[test]
fn custom_node_slots_render_the_shared_studio_facts() {
    clear_studio();
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_app(sink);
    navigate_to_proxies(&mut app);

    // Honest empty state first.
    assert_eq!(
        text_for_slot(&mut app, CustomNodeSlot::Chips),
        "尚无节点草稿"
    );
    assert_eq!(
        text_for_slot(&mut app, CustomNodeSlot::Gaps),
        "分享链接可完整表达当前草稿"
    );

    // 05-01 + 05-11: an SS-2022 draft with multiplexing overrides.
    let mut draft = ProtocolDraft::new("ss");
    draft.name = "hk-2022".to_owned();
    draft.server = "1.1.1.1".to_owned();
    draft.port = 8388;
    draft.cipher = "2022-blake3-aes-256-gcm".to_owned();
    draft.password = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_owned();
    draft.smux.enabled = true;
    draft.smux.protocol = "yamux".to_owned();
    draft.smux.max_connections = 8;
    let studio = ProtocolCodecApplication::publish_draft(draft.clone(), None);
    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(ProxiesProjection {
            groups: Vec::new(),
            testing: false,
            active_exit: "—".to_owned(),
            custom_node: studio,
        }));
    app.update();

    let chips = text_for_slot(&mut app, CustomNodeSlot::Chips);
    assert!(chips.contains("Shadowsocks"), "{chips}");
    assert!(chips.contains("2022 · AES-256-GCM"), "{chips}");
    assert!(chips.contains("PSK 32B"), "{chips}");
    assert!(chips.contains("mux:yamux"), "{chips}");
    assert!(chips.contains("mc:8"), "{chips}");
    assert_eq!(
        text_for_slot(&mut app, CustomNodeSlot::Issues),
        "协议校验通过"
    );
    // 05-11 is YAML-only; the shared application measured the gap itself.
    let gaps = text_for_slot(&mut app, CustomNodeSlot::Gaps);
    assert!(gaps.contains("smux"), "{gaps}");

    // 05-02: a REALITY + Vision VLESS draft renders its typed chips.
    let reality = ProtocolCodecApplication::draft_from_uri(VLESS_URI).unwrap();
    let preview = ProtocolCodecApplication::uri_from_draft(&reality).ok();
    let studio = ProtocolCodecApplication::publish_draft(reality, preview);
    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(ProxiesProjection {
            groups: Vec::new(),
            testing: false,
            active_exit: "—".to_owned(),
            custom_node: studio,
        }));
    app.update();

    let chips = text_for_slot(&mut app, CustomNodeSlot::Chips);
    assert!(chips.contains("VLESS"), "{chips}");
    assert!(chips.contains("Vision 流控"), "{chips}");
    assert!(chips.contains("Reality"), "{chips}");
    assert!(chips.contains("sid:abcd1234"), "{chips}");
    assert!(chips.contains("fp:chrome"), "{chips}");

    let preview = text_for_slot(&mut app, CustomNodeSlot::UriPreview);
    assert!(preview.starts_with("vless://"), "{preview}");
    assert!(preview.contains("flow=xtls-rprx-vision"), "{preview}");

    // 05-04: WireGuard parameters render from the same shared report, and the
    // honest cross-version notes have their own slot.
    let mut wg = ProtocolDraft::new("wireguard");
    wg.name = "matrix-wg".to_owned();
    wg.server = "wg.example.com".to_owned();
    wg.port = 51820;
    wg.params.wireguard.private_key = WG_KEY.to_owned();
    wg.params.wireguard.public_key = WG_KEY.to_owned();
    wg.params.wireguard.reserved = "1,2,3".to_owned();
    wg.params.wireguard.mtu = 1420;
    wg.params.wireguard.amnezia.jc = Some(4);
    wg.params.transport.network = "quic".to_owned();
    let studio = ProtocolCodecApplication::publish_draft(wg, None);
    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(ProxiesProjection {
            groups: Vec::new(),
            testing: false,
            active_exit: "—".to_owned(),
            custom_node: studio,
        }));
    app.update();

    let chips = text_for_slot(&mut app, CustomNodeSlot::Chips);
    assert!(chips.contains("wg:key"), "{chips}");
    assert!(chips.contains("wg:reserved(list)"), "{chips}");
    assert!(chips.contains("mtu:1420"), "{chips}");
    assert!(chips.contains("awg:jc=4"), "{chips}");
    let notes = text_for_slot(&mut app, CustomNodeSlot::Notes);
    assert!(notes.contains("无跨版本提示"), "{notes}");
}

#[test]
fn custom_node_notes_slot_reports_pinned_core_fallbacks_verbatim() {
    clear_studio();
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_app(sink);
    navigate_to_proxies(&mut app);

    // The pinned mihomo v1.19.18 has no `quic` network and no xhttp transport:
    // the shared report says so and Bevy renders the note instead of a claim.
    let mut draft = ProtocolDraft::new("vless");
    draft.name = "quic-node".to_owned();
    draft.server = "example.com".to_owned();
    draft.port = 443;
    draft.uuid = "b831381d-6324-4d53-ad4f-8cda48b30811".to_owned();
    draft.params.transport.network = "quic".to_owned();
    draft.params.transport.xhttp.mode = "stream-up".to_owned();
    let studio = ProtocolCodecApplication::publish_draft(draft, None);
    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(ProxiesProjection {
            groups: Vec::new(),
            testing: false,
            active_exit: "—".to_owned(),
            custom_node: studio,
        }));
    app.update();

    let notes = text_for_slot(&mut app, CustomNodeSlot::Notes);
    assert!(notes.contains("v1.19.18"), "{notes}");
    assert!(notes.contains("TCP"), "{notes}");
}

#[test]
fn protocol_codec_matrix_passes_on_the_bevy_surface() {
    let report = infiltrator_application::protocol_codec_matrix_application::ProtocolCodecMatrixApplication::run_deterministic_matrix();
    assert!(
        report.all_covered_passed(),
        "matrix failures: {:?}",
        report.failed_ids()
    );
    assert_eq!(report.covered_passed_count(), 15);
    assert!(
        report.summary_zh().contains("15/15"),
        "{}",
        report.summary_zh()
    );
}

#[test]
fn custom_node_audit_line_reports_the_measured_lossless_verdict() {
    clear_studio();
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_app(sink);
    navigate_to_proxies(&mut app);

    // 05-14: the shared application audits a real conversion and publishes the
    // verdict; Bevy renders it verbatim instead of claiming success.
    let profile = "proxies:\n  - name: a\n    type: ss\n    server: 1.1.1.1\n    port: 443\n    cipher: aes-128-gcm\n    password: pw\n    future-flag: 7\nmode: rule\n";
    let (_output, audit) = ProtocolCodecApplication::audit_conversion(
        profile,
        infiltrator_contract::protocol_fidelity::NodeCodecFormat::ClashYaml,
        infiltrator_contract::protocol_fidelity::NodeCodecFormat::ClashYaml,
    )
    .unwrap();
    assert!(
        !audit.lossless,
        "a profile with `mode:` is not section-preserving"
    );

    let live = infiltrator_contract::protocol_fidelity::ProtocolStudioSnapshot {
        audit: Some(audit),
        ..Default::default()
    };
    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(ProxiesProjection {
            groups: Vec::new(),
            testing: false,
            active_exit: "—".to_owned(),
            custom_node: live,
        }));
    app.update();

    let audit_line = text_for_slot(&mut app, CustomNodeSlot::Audit);
    assert!(audit_line.contains("结构有损"), "{audit_line}");
    assert!(audit_line.contains("1 节点"), "{audit_line}");
    assert!(audit_line.contains("未知字段 1"), "{audit_line}");
}

#[test]
fn custom_node_dialer_and_ca_slots_render_the_shared_facts() {
    clear_studio();
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_app(sink);
    navigate_to_proxies(&mut app);

    // Honest empty states before anything is published.
    assert_eq!(
        text_for_slot(&mut app, CustomNodeSlot::Chain),
        "无前置跳板链路"
    );
    assert_eq!(
        text_for_slot(&mut app, CustomNodeSlot::CaTrust),
        "未配置自定义证书信任"
    );

    // 05-09: a real hop chain resolved by the shared analyzer.
    let profile = "proxies:\n  - name: nas\n    type: ss\n    server: 1.1.1.1\n    port: 8388\n    cipher: aes-128-gcm\n    password: pw\n    dialer-proxy: gateway\n  - name: gateway\n    type: vless\n    server: 2.2.2.2\n    port: 443\n    uuid: b831381d-6324-4d53-ad4f-8cda48b30811\n";
    let report = ProtocolCodecApplication::publish_dialer_report(profile).expect("dialer report");
    let mut draft = ProtocolCodecApplication::draft_from_uri(VLESS_URI).unwrap();
    draft.name = "nas".to_owned();
    draft.dialer_proxy = "gateway".to_owned();
    let studio = ProtocolCodecApplication::publish_draft(draft, None);
    assert_eq!(studio.dialer, report);
    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(ProxiesProjection {
            groups: Vec::new(),
            testing: false,
            active_exit: "—".to_owned(),
            custom_node: studio,
        }));
    app.update();

    let chain = text_for_slot(&mut app, CustomNodeSlot::Chain);
    assert!(chain.contains("nas(Shadowsocks)"), "{chain}");
    assert!(chain.contains("gateway(VLESS)"), "{chain}");
    assert!(chain.contains("链路完整"), "{chain}");

    // 05-10: a loop is rendered as a loop warning, never as a valid chain.
    let loop_profile = "proxies:\n  - name: a\n    type: ss\n    server: 1.1.1.1\n    port: 8388\n    cipher: aes-128-gcm\n    password: pw\n    dialer-proxy: b\n  - name: b\n    type: ss\n    server: 2.2.2.2\n    port: 8388\n    cipher: aes-128-gcm\n    password: pw\n    dialer-proxy: a\n";
    ProtocolCodecApplication::publish_dialer_report(loop_profile).expect("loop report");
    let mut loop_draft = ProtocolCodecApplication::draft_from_uri(VLESS_URI).unwrap();
    loop_draft.name = "a".to_owned();
    loop_draft.dialer_proxy = "b".to_owned();
    let studio = ProtocolCodecApplication::publish_draft(loop_draft, None);
    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(ProxiesProjection {
            groups: Vec::new(),
            testing: false,
            active_exit: "—".to_owned(),
            custom_node: studio,
        }));
    app.update();
    let chain = text_for_slot(&mut app, CustomNodeSlot::Chain);
    assert!(chain.contains("环路"), "{chain}");
    assert!(
        chain.contains("⛔") || chain.contains("该链路不可用"),
        "{chain}"
    );

    // 05-13: the CA slot renders what the host really reported. A host without
    // a reader must never look loaded.
    let mut ca_draft = ProtocolCodecApplication::draft_from_uri(VLESS_URI).unwrap();
    ca_draft.params.tls_trust.ca_path = "/etc/ssl/custom-ca.pem".to_owned();
    let studio = ProtocolCodecApplication::publish_draft(ca_draft, None);
    let report = ProtocolCodecApplication::publish_ca_trust(
        &studio.draft.as_ref().expect("draft").params.tls_trust,
        None,
    );
    assert!(report.is_unsupported());
    let studio = infiltrator_contract::protocol_fidelity::ProtocolStudioSnapshot {
        ca_trust: report,
        ..studio
    };
    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(ProxiesProjection {
            groups: Vec::new(),
            testing: false,
            active_exit: "—".to_owned(),
            custom_node: studio,
        }));
    app.update();
    let ca = text_for_slot(&mut app, CustomNodeSlot::CaTrust);
    assert!(ca.contains("宿主不支持"), "{ca}");
    assert!(
        ca.contains("未加载") || ca.contains("was not loaded"),
        "{ca}"
    );
    clear_studio();
}

#[test]
fn custom_node_scan_and_ca_buttons_submit_the_shared_commands() {
    clear_studio();
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_app(Arc::clone(&sink));
    navigate_to_proxies(&mut app);

    // Publish a draft and project it, so the card has typed facts to edit.
    let draft = ProtocolCodecApplication::draft_from_uri(VLESS_URI).unwrap();
    let studio = ProtocolCodecApplication::publish_draft(draft, None);
    app.world_mut()
        .commands()
        .trigger(ProxiesProjectionUpdated(ProxiesProjection {
            groups: Vec::new(),
            testing: false,
            active_exit: "—".to_owned(),
            custom_node: studio,
        }));
    app.update();

    // Type into the hop + CA fields and activate the shared actions.
    let field_of = |app: &mut App, marker: &'static str| -> Entity {
        match marker {
            "dialer" => {
                let mut query = app
                    .world_mut()
                    .query::<(&CustomNodeDialerField, &Children)>();
                *query
                    .single(app.world())
                    .expect("dialer field")
                    .1
                    .iter()
                    .next()
                    .unwrap()
            }
            _ => {
                let mut query = app.world_mut().query::<(&CustomNodeCaField, &Children)>();
                *query
                    .single(app.world())
                    .expect("ca field")
                    .1
                    .iter()
                    .next()
                    .unwrap()
            }
        }
    };
    let dialer_field = field_of(&mut app, "dialer");
    let ca_field = field_of(&mut app, "ca");
    app.world_mut()
        .get_mut::<TextField>(dialer_field)
        .expect("dialer state")
        .0
        .apply(TextFieldInput::SetText("gateway".to_owned()));
    app.world_mut()
        .get_mut::<TextField>(ca_field)
        .expect("ca state")
        .0
        .apply(TextFieldInput::SetText("/etc/ssl/ca.pem".to_owned()));

    let scan = button_entity::<ScanDialerChainsButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: scan });
    app.update();
    assert!(
        sink.submitted().iter().any(|command| matches!(
            command,
            UiCommand::UpdateCustomNodeDraftField { field, value }
                if field == "dialer-proxy" && value == "gateway"
        )),
        "{:?}",
        sink.submitted()
    );
    assert!(
        sink.submitted()
            .iter()
            .any(|command| matches!(command, UiCommand::ScanCustomNodeDialer)),
        "{:?}",
        sink.submitted()
    );

    let verify = button_entity::<VerifyCustomNodeCaButton>(&mut app);
    app.world_mut()
        .commands()
        .trigger(Activate { entity: verify });
    app.update();
    assert!(
        sink.submitted().iter().any(|command| matches!(
            command,
            UiCommand::VerifyCustomNodeCa { trust } if trust.ca_path == "/etc/ssl/ca.pem"
        )),
        "{:?}",
        sink.submitted()
    );
    clear_studio();
}
