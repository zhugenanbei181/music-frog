//! Windows/macOS tray backend: the existing muda + tray-icon implementation,
//! wrapped behind the neutral [`TrayController`]/[`TraySpec`] seam. Since 0.20
//! the menu is fully generic: every [`update_spec`] push clears the root menu
//! and re-renders the whole [`TrayMenuItem`] tree from the spec (same source
//! of truth as the ksni backend), and the tooltip is refreshed too.
//!
//! Compiles on Linux too (muda/tray-icon keep the gtk feature at the
//! workspace root), which is how this backend stays checkable on this OS.
//!
//! # Event-handler lifecycle (contract §2.1)
//!
//! muda 0.19 (Cargo.lock) exposes process-global, install-once handlers:
//! `MenuEvent::set_event_handler` / `TrayIconEvent::set_event_handler` back
//! onto a `OnceCell<Option<_>>`, so a second `Some(...)` is a silent no-op
//! and `None` can never clear an installed closure. To keep repeated
//! spawn/refresh cycles from accumulating handlers (or from silently
//! attaching to a dead sender), this backend installs each global handler
//! **exactly once per process** and routes events through a swappable sink
//! ([`EVENT_SINK`]): every spawn replaces the sink's sender, and
//! [`TrayController::shutdown`] detaches it (generation-checked) and drops
//! the [`TrayIcon`], whose `Drop` unregisters the OS icon on all platforms.

use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, Once};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use super::spec::{
    TRAY_ACTION_NO_PROXIES, TrayActionId, TrayController, TrayEvent, TrayIconData, TrayMenuItem,
    TraySpec, TrayStartup,
};

pub(super) struct TrayManager {
    tray_icon: Option<TrayIcon>,
    menu: Menu,
}

impl TrayManager {
    pub fn new(icon: Option<&TrayIconData>) -> Self {
        // The menu content is driven entirely by apply_spec; spawn starts
        // with an empty menu so there is exactly one renderer of state.
        let menu = Menu::new();
        let mut builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
            .with_tooltip("MusicFrog Infiltrator");

        // macOS 左键语义（平台契约 §2.1 的显式决策）：**左键单击 = 显示主
        // 窗口**（经 [`TrayEvent::IconActivated`] 走既有 `ShowWindow` 意图，
        // 与 Clash Party/Clash Verge 的托盘习惯一致）；菜单留给右键 / 状态
        // 栏默认行为 —— tray-icon 的 `menu_on_right_click` 默认开启，关掉
        // 左键菜单后右键仍会弹出。不设此项时 macOS 左键只弹菜单，几乎不会
        // 派发 Click 事件，`ShowWindow` 入口形同虚设。
        // Windows/macOS 之外的编译目标（本文件仅为可检性在 Linux 编译）
        // 保持 tray-icon 默认行为不变。
        #[cfg(target_os = "macos")]
        {
            builder = builder.with_menu_on_left_click(false);
        }

        if let Some(spec_icon) = icon
            && let Ok(i) =
                Icon::from_rgba(spec_icon.rgba.clone(), spec_icon.width, spec_icon.height)
        {
            builder = builder.with_icon(i);
        }

        // 这里的 build() 可能因为系统不支持或资源冲突失败，包装在 Option 中
        let tray_icon = builder.build().ok();

        Self { tray_icon, menu }
    }

    /// Re-render the imperative muda menu from the neutral spec tree and
    /// refresh the tooltip. The spec is the single source of truth: labels
    /// (including `● ` markers and disabled placeholders), checkmark states,
    /// enabled flags and payloads all come straight from the spec builder.
    pub fn apply_spec(&self, spec: &TraySpec) {
        // A freshly pushed spec is authoritative: drop any click-time
        // checkmark overrides, exactly like the ksni backend's
        // `KsniController::update_spec` clears `checked_overrides`.
        lock(&CHECK_OVERRIDES).clear();

        // Rebuild both checkmark registries from the new tree: the
        // pure-data one feeds the install-once event handler (it may only
        // touch Send state), the thread-local one holds the live
        // [`CheckMenuItem`] handles for the click-time visual echo.
        lock(&CHECK_REGISTRY).clear();
        LIVE_CHECK_ITEMS.with(|items| items.borrow_mut().clear());

        while self.menu.remove_at(0).is_some() {}
        for item in &spec.menu.items {
            if let Some(entry) = build_entry(item) {
                let _ = append_entry_to_menu(&self.menu, &entry);
            }
        }
        if let Some(icon) = &self.tray_icon {
            let _ = icon.set_tooltip(Some(spec.tooltip.as_str()));
        }
    }
}

/// Composite checkmark-override key mirrored from the ksni backend:
/// (action id, payload echo). Keying by id *and* payload keeps per-item
/// checkmarks that share one id (e.g. one auto-update toggle per profile)
/// from crosstalking.
type CheckedOverrideKey = (TrayActionId, Option<String>);

/// Registry entry for one rendered checkmark: its override key plus the
/// checkmark state as last displayed.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CheckEntry {
    key: CheckedOverrideKey,
    checked: bool,
}

/// Current checkmark entries of the live menu, keyed by the muda wire id.
/// Pure `Send` data because the global muda handler ([`handle_menu_event`])
/// consults and mutates it on click.
static CHECK_REGISTRY: LazyLock<Mutex<HashMap<String, CheckEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Click-time checkmark overrides, same semantics as `KsniTray::
/// checked_overrides`: some shells flip their own checkbox and the fresh
/// spec only arrives one event-loop round later, so the clicked state must
/// survive until the next `apply_spec` clears it.
static CHECK_OVERRIDES: LazyLock<Mutex<HashMap<CheckedOverrideKey, bool>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

thread_local! {
    /// Live [`CheckMenuItem`] clones keyed by muda wire id, recorded while
    /// `apply_spec` renders on the main thread. The install-once muda
    /// handler also fires on the main UI thread on every supported platform
    /// (Windows message pump, macOS main queue), so the click-time
    /// `set_checked` below flips the visible checkmark immediately. On any
    /// backend that ever dispatches menu events off-thread the lookup simply
    /// misses and the flip lands on the next `apply_spec` rebuild instead —
    /// never a panic.
    static LIVE_CHECK_ITEMS: RefCell<HashMap<String, CheckMenuItem>> =
        RefCell::new(HashMap::new());
}

/// Pure core of the click-time checkmark flip (unit-tested headlessly).
/// Resolves the currently displayed state (override first, then the state
/// recorded at render time), records the flipped override and updates the
/// registry entry so back-to-back clicks keep toggling even before any new
/// spec arrives. Returns the override key plus the new state, or `None` for
/// foreign/stale muda ids (nothing is mutated then).
fn flip_checkmark_in(
    registry: &mut HashMap<String, CheckEntry>,
    overrides: &mut HashMap<CheckedOverrideKey, bool>,
    muda_id: &str,
) -> Option<(CheckedOverrideKey, bool)> {
    let entry = registry.get_mut(muda_id)?;
    let current = overrides.get(&entry.key).copied().unwrap_or(entry.checked);
    let flipped = !current;
    overrides.insert(entry.key.clone(), flipped);
    entry.checked = flipped;
    Some((entry.key.clone(), flipped))
}

/// Immediate visual echo of a click-time flip: flip the live muda checkbox
/// on the thread that owns the menu (see [`LIVE_CHECK_ITEMS`]). Best-effort;
/// a miss defers the flip to the next rebuild.
fn echo_checkmark(muda_id: &str, checked: bool) {
    LIVE_CHECK_ITEMS.with(|items| {
        if let Some(item) = items.borrow().get(muda_id) {
            item.set_checked(checked);
        }
    });
}

/// Register one rendered checkmark in both registries.
fn register_check_entry(muda_id: &str, key: CheckedOverrideKey, checked: bool, item: CheckMenuItem) {
    lock(&CHECK_REGISTRY).insert(muda_id.to_string(), CheckEntry { key, checked });
    LIVE_CHECK_ITEMS.with(|items| {
        items.borrow_mut().insert(muda_id.to_string(), item);
    });
}

/// Poison-tolerant lock: a panic while some code holds a lock must not take
/// the tray (or a click) down with it.
fn lock<T>(cell: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    cell.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A concrete muda widget built from one neutral spec entry. Kept as an enum
/// because `Menu` and `Submenu` share no append-capable trait.
enum MudaEntry {
    Command(MenuItem),
    Check(CheckMenuItem),
    Submenu(Submenu),
    Separator(PredefinedMenuItem),
}

fn build_entry(item: &TrayMenuItem) -> Option<MudaEntry> {
    match item {
        TrayMenuItem::Action {
            id,
            label,
            enabled,
            payload,
        } => Some(MudaEntry::Command(MenuItem::with_id(
            muda_id_for(*id, payload.as_deref()),
            label,
            *enabled,
            None,
        ))),
        TrayMenuItem::Checkmark {
            id,
            label,
            checked,
            enabled,
            payload,
        } => {
            let muda_id = muda_id_for(*id, payload.as_deref());
            let item = CheckMenuItem::with_id(muda_id.clone(), label, *checked, *enabled, None);
            register_check_entry(&muda_id, (*id, payload.clone()), *checked, item.clone());
            Some(MudaEntry::Check(item))
        }
        TrayMenuItem::Submenu {
            id,
            label,
            enabled,
            items,
        } => {
            let submenu = Submenu::with_id(muda_id_for(*id, None), label, *enabled);
            append_entries(&submenu, items);
            Some(MudaEntry::Submenu(submenu))
        }
        TrayMenuItem::Separator => Some(MudaEntry::Separator(PredefinedMenuItem::separator())),
    }
}

fn append_entries(target: &Submenu, items: &[TrayMenuItem]) {
    for item in items {
        if let Some(entry) = build_entry(item) {
            let _ = append_entry_to_submenu(target, &entry);
        }
    }
}

fn append_entry_to_menu(target: &Menu, entry: &MudaEntry) -> muda::Result<()> {
    match entry {
        MudaEntry::Command(item) => target.append(item),
        MudaEntry::Check(item) => target.append(item),
        MudaEntry::Submenu(item) => target.append(item),
        MudaEntry::Separator(item) => target.append(item),
    }
}

fn append_entry_to_submenu(target: &Submenu, entry: &MudaEntry) -> muda::Result<()> {
    match entry {
        MudaEntry::Command(item) => target.append(item),
        MudaEntry::Check(item) => target.append(item),
        MudaEntry::Submenu(item) => target.append(item),
        MudaEntry::Separator(item) => target.append(item),
    }
}

/// muda menu ids are strings. The wire format mirrors the neutral id/payload
/// pair: `tray-action-<id>` for payload-less entries and
/// `tray-action-<id>:<payload>` otherwise (payloads may themselves contain
/// `:`, only the first separator splits). Submenu ids use the same encoding
/// for traceability; they are never emitted as activations.
fn muda_id_for(id: TrayActionId, payload: Option<&str>) -> String {
    match payload {
        Some(payload) => format!("tray-action-{id}:{payload}"),
        None => format!("tray-action-{id}"),
    }
}

/// Translate a muda menu id back into the neutral event. Unparseable ids
/// (foreign entries, races with menu teardown) fall back to the disabled
/// placeholder id, which resolves to no intent.
fn translate_menu_id(id: &str) -> TrayEvent {
    let parsed = id.strip_prefix("tray-action-").and_then(|rest| {
        let (action, payload) = match rest.split_once(':') {
            Some((action, payload)) => (action, Some(payload.to_string())),
            None => (rest, None),
        };
        action
            .parse::<TrayActionId>()
            .ok()
            .map(|action| (action, payload))
    });
    match parsed {
        Some((action, payload)) => TrayEvent::MenuActivated {
            id: action,
            payload,
        },
        None => TrayEvent::MenuActivated {
            id: TRAY_ACTION_NO_PROXIES,
            payload: None,
        },
    }
}

/// Routing slot for the install-once handlers: whichever tray spawned last
/// owns the event stream. Replacing the sink drops the previous sender, so
/// repeated spawn/refresh cycles never accumulate closures or senders.
struct EventSink {
    sender: Sender<TrayEvent>,
    generation: u64,
}

static EVENT_SINK: Mutex<Option<EventSink>> = Mutex::new(None);
static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
static EVENT_HANDLERS: Once = Once::new();

/// Install the process-global muda/tray-icon handlers exactly once (see the
/// module docs for why they can neither be replaced nor cleared afterwards),
/// then route through the current [`EVENT_SINK`].
fn install_event_handlers_once() {
    EVENT_HANDLERS.call_once(|| {
        muda::MenuEvent::set_event_handler(Some(|event: muda::MenuEvent| {
            handle_menu_event(event);
        }));
        tray_icon::TrayIconEvent::set_event_handler(Some(|event| {
            handle_tray_icon_event(event);
        }));
    });
}

/// Forward one neutral tray event to the current tray's channel; dropped
/// when no tray is live (post-shutdown) or the receiver is gone.
fn forward_event(event: TrayEvent) {
    let sink = lock(&EVENT_SINK);
    if let Some(sink) = sink.as_ref() {
        let _ = sink.sender.send(event);
    }
}

/// muda menu click: flip the checkmark display first (contract §2.1, same
/// semantics as the ksni `checked_overrides`), then forward the neutral
/// event so the app applies the intent and pushes the authoritative spec.
fn handle_menu_event(event: muda::MenuEvent) {
    let muda_id = event.id.as_ref();
    let flipped = {
        let mut registry = lock(&CHECK_REGISTRY);
        let mut overrides = lock(&CHECK_OVERRIDES);
        flip_checkmark_in(&mut registry, &mut overrides, muda_id)
    };
    if let Some((_, checked)) = flipped {
        echo_checkmark(muda_id, checked);
    }
    forward_event(translate_menu_id(muda_id));
}

/// Tray icon click semantics: **left click = show the main window**
/// ([`TrayEvent::IconActivated`], see the macOS decision in
/// [`TrayManager::new`]). Right clicks stay with the system default (open
/// the menu) and must not raise the window — on macOS `rightMouseDown`
/// dispatches a `Click` event too, hence the button filter.
fn handle_tray_icon_event(event: tray_icon::TrayIconEvent) {
    if let tray_icon::TrayIconEvent::Click {
        button: tray_icon::MouseButton::Left,
        ..
    } = event
    {
        forward_event(TrayEvent::IconActivated);
    }
}

struct NativeTrayController {
    manager: TrayManager,
    generation: u64,
}

impl TrayController for NativeTrayController {
    fn update_spec(&self, spec: TraySpec) {
        self.manager.apply_spec(&spec);
    }

    fn shutdown(&mut self) {
        // Real teardown, not just a cosmetic hide: dropping the TrayIcon
        // unregisters the OS icon on every platform (tray-icon 0.24 Drop
        // impls); `set_visible(false)` alone would leave the NSStatusItem
        // (or Windows notification-area entry) behind.
        self.manager.tray_icon = None;
        // Detach the event routing — but only while this controller is
        // still the incumbent tray, so a stale controller shutting down
        // after a respawn cannot cut the new tray's event stream. The
        // muda-global handler itself stays installed (OnceCell, see module
        // docs); with no sink its events are simply dropped.
        let mut sink = lock(&EVENT_SINK);
        if sink.as_ref().is_some_and(|s| s.generation == self.generation) {
            *sink = None;
        }
    }
}

/// Spawn the muda/tray-icon tray and forward its native event streams into
/// the neutral channel. `builder.build()` failure (unsupported system,
/// resource conflict) degrades to a typed [`TrayStartup::Unavailable`].
pub(super) fn spawn_native(spec: TraySpec) -> TrayStartup {
    let (events, receiver) = std::sync::mpsc::channel();
    let manager = TrayManager::new(spec.icon.as_ref());

    if manager.tray_icon.is_none() {
        return TrayStartup::Unavailable {
            reason: "tray_icon build failed (unsupported system or resource conflict)".to_string(),
        };
    }

    install_event_handlers_once();
    let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
    // Replaces any previous sink; the old sender drops with it, so a
    // respawned tray inherits the stream and nothing accumulates.
    *lock(&EVENT_SINK) = Some(EventSink { sender: events, generation });

    manager.apply_spec(&spec);

    TrayStartup::Ready {
        controller: Box::new(NativeTrayController { manager, generation }),
        events: receiver,
    }
}

#[cfg(test)]
mod id_codec {
    use super::*;

    #[test]
    fn muda_id_round_trips_action_and_payload() {
        // Payload-less actions round-trip bare.
        assert_eq!(muda_id_for(7, None), "tray-action-7");
        let TrayEvent::MenuActivated { id, payload } = translate_menu_id("tray-action-7") else {
            panic!("must parse into a menu activation");
        };
        assert_eq!((id, payload.as_deref()), (7, None));

        // Payload-carrying actions keep the payload verbatim, including
        // `:` and the `\u{1}` pair separator.
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

        // Numeric-looking garbage with a bad action part also falls back.
        let TrayEvent::MenuActivated { id, .. } = translate_menu_id("tray-action-99999:big") else {
            panic!("fallback must still be a menu activation");
        };
        assert_eq!(id, TRAY_ACTION_NO_PROXIES);
    }
}

#[cfg(test)]
mod check_overrides {
    use super::*;

    fn entry(muda_id: &str, key: CheckedOverrideKey, checked: bool) -> (String, CheckEntry) {
        (muda_id.to_string(), CheckEntry { key, checked })
    }

    #[test]
    fn flip_toggles_display_state_and_records_override() {
        let mut registry = HashMap::from([entry(
            "tray-action-7",
            (7, None),
            false, // spec said unchecked at render time
        )]);
        let mut overrides = HashMap::new();

        // First click flips false -> true and records the override so the
        // state survives until the next spec push.
        assert_eq!(
            flip_checkmark_in(&mut registry, &mut overrides, "tray-action-7"),
            Some(((7, None), true))
        );
        assert_eq!(overrides.get(&(7, None)), Some(&true));
        assert!(registry["tray-action-7"].checked);

        // A second click (no spec in between) keeps toggling, resolving the
        // current state from the override first.
        assert_eq!(
            flip_checkmark_in(&mut registry, &mut overrides, "tray-action-7"),
            Some(((7, None), false))
        );
        assert_eq!(overrides.get(&(7, None)), Some(&false));
    }

    #[test]
    fn flip_resolves_payload_scoped_keys_without_crosstalk() {
        // Same action id, two payloads (per-profile toggles): overrides must
        // never leak across the payload boundary.
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
            flip_checkmark_in(&mut registry, &mut overrides, &format!("tray-action-9:{payload_a}")),
            Some(((9, Some(payload_a.clone())), true))
        );
        // The other payload-scoped entry keeps its rendered state.
        assert_eq!(
            flip_checkmark_in(&mut registry, &mut overrides, &format!("tray-action-9:{payload_b}")),
            Some(((9, Some(payload_b.clone())), false))
        );
        assert_eq!(overrides.len(), 2);
    }

    #[test]
    fn flip_ignores_foreign_and_stale_ids_without_mutation() {
        // Removed items leave stale ids in the handler's view until the next
        // apply_spec clears the registry; clicking them must be a no-op.
        let mut registry = HashMap::from([entry("tray-action-3", (3, None), true)]);
        let mut overrides = HashMap::new();

        assert_eq!(flip_checkmark_in(&mut registry, &mut overrides, "not-ours"), None);
        assert_eq!(flip_checkmark_in(&mut registry, &mut overrides, "tray-action-99999:x"), None);
        assert!(overrides.is_empty());
        assert!(registry["tray-action-3"].checked);
    }

    #[test]
    fn clear_overrides_semantics_match_ksni_spec_push() {
        // apply_spec clears the override store before rendering, so the next
        // flip must resolve from the freshly rendered state again — never
        // from a stale override of a previous generation of the menu.
        let mut overrides: HashMap<CheckedOverrideKey, bool> = HashMap::from([((4, None), true)]);
        let mut registry = HashMap::from([entry("tray-action-4", (4, None), false)]);

        // Before the spec push the click would resolve `true` from the
        // stale override and flip to `false`:
        assert_eq!(flip_checkmark_in(&mut registry, &mut overrides, "tray-action-4"),
            Some(((4, None), false)));

        // ... but a spec push clears overrides first (apply_spec), so the
        // same click resolves the authoritative rendered state instead:
        overrides.clear();
        registry.get_mut("tray-action-4").unwrap().checked = false; // re-rendered unchecked
        assert_eq!(flip_checkmark_in(&mut registry, &mut overrides, "tray-action-4"),
            Some(((4, None), true)));
    }
}
