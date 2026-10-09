//! BEVY-037 / BEVY-041: isolated third-party widget dashboard slot.
//!
//! A community micro-frontend widget is attached through a typed ABI
//! negotiation and then runs behind the [`SandboxedWidgetInstance`] isolation
//! container. The host owns the [`IsolatedRenderSlot`] and resolves it by id
//! when compositing; the guest never receives an entity, component or `World`.
//! A denied capability is surfaced as a typed value, never a panic or a silent
//! grant, and a version/capability mismatch is reported before any attach.

use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut};
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::prelude::{
    BackgroundColor, BorderColor, BorderRadius, FlexDirection, Node, PositionType, UiRect, Val, px,
};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::abi::{
    AbiNegotiation, HostCapabilities, WIDGET_ABI_VERSION, WidgetAbiRequirement, WidgetCapability,
    negotiate,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::sandbox::{
    HostCapability, IsolatedRenderSlot, SandboxFault, SandboxedWidgetInstance, WidgetManifest,
};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// Isolated render-slot width the host assigns to a dashboard widget.
pub const WIDGET_SLOT_WIDTH: u32 = 320;
/// Isolated render-slot height the host assigns to a dashboard widget.
pub const WIDGET_SLOT_HEIGHT: u32 = 180;

/// Host ABI advertisement used for attach-time negotiation.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidgetHostAbi {
    pub version: (u32, u32, u32),
    pub capabilities: HostCapabilities,
}

impl Default for WidgetHostAbi {
    fn default() -> Self {
        Self {
            version: WIDGET_ABI_VERSION,
            capabilities: HostCapabilities::all_desktop(),
        }
    }
}

/// Typed result of attaching one widget to the dashboard slot.
#[derive(Clone, Debug, PartialEq)]
pub enum WidgetAttachOutcome {
    /// The widget negotiated, validated and attached to an isolated slot.
    Attached {
        widget_id: String,
        slot: IsolatedRenderSlot,
        granted: HostCapabilities,
    },
    /// The sandbox container refused the widget (manifest / quota / permission).
    Rejected(SandboxFault),
    /// The host ABI cannot run the widget; there is no silent downgrade.
    Incompatible(AbiNegotiation),
}

impl WidgetAttachOutcome {
    pub fn is_attached(&self) -> bool {
        matches!(self, WidgetAttachOutcome::Attached { .. })
    }

    pub fn incompatibility(&self) -> Option<AbiNegotiation> {
        match self {
            WidgetAttachOutcome::Incompatible(negotiation) => Some(*negotiation),
            _ => None,
        }
    }
}

/// The dashboard slot's attached widgets and their typed attach history.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct WidgetSandboxBoard {
    instances: Vec<SandboxedWidgetInstance>,
    outcomes: Vec<WidgetAttachOutcome>,
    last_denial: Option<(HostCapability, SandboxFault)>,
    denial_count: u64,
    next_slot_id: u32,
}

impl WidgetSandboxBoard {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn instances(&self) -> &[SandboxedWidgetInstance] {
        &self.instances
    }

    pub fn outcomes(&self) -> &[WidgetAttachOutcome] {
        &self.outcomes
    }

    pub fn attached_count(&self) -> usize {
        self.instances.len()
    }

    pub fn last_outcome(&self) -> Option<&WidgetAttachOutcome> {
        self.outcomes.last()
    }

    pub fn first_attached(&self) -> Option<&SandboxedWidgetInstance> {
        self.instances.first()
    }

    pub fn denial_count(&self) -> u64 {
        self.denial_count
    }

    /// The capability denied by the most recent authorization, if any.
    pub fn denied_capability(&self) -> Option<HostCapability> {
        self.last_denial.map(|(capability, _)| capability)
    }

    /// Attach a widget through ABI negotiation and the isolation container.
    ///
    /// The order is deliberate: an incompatible host version or a missing
    /// capability is reported before the manifest is ever bound, so a mismatch
    /// can never be mistaken for a successful attach.
    pub fn attach(
        &mut self,
        host: WidgetHostAbi,
        manifest: &WidgetManifest,
        requirement: WidgetAbiRequirement,
    ) -> WidgetAttachOutcome {
        let negotiation = negotiate(host.version, host.capabilities, requirement);
        if !negotiation.is_compatible() {
            return self.record(WidgetAttachOutcome::Incompatible(negotiation));
        }
        let mut instance = SandboxedWidgetInstance::new(manifest.id.clone(), manifest.name.clone());
        instance.version = manifest.version.clone();
        if let Err(fault) = instance.bind_manifest(manifest) {
            return self.record(WidgetAttachOutcome::Rejected(fault));
        }
        let slot =
            IsolatedRenderSlot::new(self.next_slot_id, WIDGET_SLOT_WIDTH, WIDGET_SLOT_HEIGHT);
        self.next_slot_id += 1;
        instance.assign_render_slot(slot);
        let granted = negotiation.granted().unwrap_or_else(HostCapabilities::none);
        self.instances.push(instance);
        self.record(WidgetAttachOutcome::Attached {
            widget_id: manifest.id.clone(),
            slot,
            granted,
        })
    }

    /// Exercise one host capability on the first attached widget.
    ///
    /// A refusal is recorded as a typed denial rather than swallowed; a
    /// container with no attached widget refuses with the same typed reason.
    pub fn authorize_capability(&mut self, capability: HostCapability) -> Result<(), SandboxFault> {
        let Some(widget) = self.instances.first() else {
            return Err(SandboxFault::CapabilityDenied(capability));
        };
        match widget.authorize(capability) {
            Ok(_) => Ok(()),
            Err(fault) => {
                self.last_denial = Some((capability, fault));
                self.denial_count += 1;
                Err(fault)
            }
        }
    }

    fn record(&mut self, outcome: WidgetAttachOutcome) -> WidgetAttachOutcome {
        self.outcomes.push(outcome.clone());
        outcome
    }
}

/// Request to attach a widget to the dashboard slot.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct AttachSandboxWidget {
    pub manifest: WidgetManifest,
    pub requirement: WidgetAbiRequirement,
}

impl AttachSandboxWidget {
    pub fn new(manifest: WidgetManifest, requirement: WidgetAbiRequirement) -> Self {
        Self {
            manifest,
            requirement,
        }
    }
}

/// Request to exercise one host capability on the attached dashboard widget.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthorizeSandboxCapability(pub HostCapability);

/// Whether the dashboard slot overlay is mounted.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WidgetSandboxVisibility(pub bool);

/// Mount latch for the dashboard slot overlay.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WidgetSandboxMounted(pub bool);

/// Marker on the mounted dashboard slot root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WidgetSandboxSlotRoot;

/// The mutable text slots of the mounted dashboard slot.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WidgetSandboxText(pub WidgetSandboxTextKind);

/// One restampable text line of the dashboard slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WidgetSandboxTextKind {
    #[default]
    Title,
    Identity,
    Slot,
    Quota,
    Grant,
    Status,
}

/// Observer: attach a requested widget through the typed pipeline.
///
/// A successful attach reveals the dashboard slot; a rejected/incompatible
/// attach leaves it hidden, so the slot never shows a non-widget.
pub fn on_attach_sandbox_widget(
    attach: On<AttachSandboxWidget>,
    host: Res<WidgetHostAbi>,
    mut board: ResMut<WidgetSandboxBoard>,
    mut visibility: ResMut<WidgetSandboxVisibility>,
) {
    let event = attach.event();
    let outcome = board.attach(*host, &event.manifest, event.requirement);
    if outcome.is_attached() {
        visibility.0 = true;
    }
}

/// Observer: authorize a host capability and record a typed denial.
pub fn on_authorize_sandbox_capability(
    request: On<AuthorizeSandboxCapability>,
    mut board: ResMut<WidgetSandboxBoard>,
) {
    let _ = board.authorize_capability(request.0);
}

/// Mount / unmount the dashboard slot overlay when its visibility changes.
pub fn sync_widget_sandbox_slot(
    mut commands: Commands,
    palette: Option<Res<UiPalette>>,
    visibility: Res<WidgetSandboxVisibility>,
    board: Res<WidgetSandboxBoard>,
    mut mounted: ResMut<WidgetSandboxMounted>,
    roots: Query<Entity, With<WidgetSandboxSlotRoot>>,
) {
    let Some(palette) = palette else {
        return;
    };
    if mounted.0 == visibility.0 {
        return;
    }
    mounted.0 = visibility.0;
    if !visibility.0 {
        for entity in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    if roots.is_empty() {
        commands.spawn_scene(widget_sandbox_slot_scene(&board, &palette));
    }
}

/// Restamp the mounted dashboard slot from the live sandbox board.
pub fn refresh_widget_sandbox_slot(
    board: Res<WidgetSandboxBoard>,
    mut texts: Query<(&mut Text, &WidgetSandboxText)>,
) {
    for (mut text, kind) in &mut texts {
        let value = sandbox_readout(&board, kind.0);
        if text.0 != value {
            text.0 = value;
        }
    }
}

/// The dashboard slot scene: every value is a typed readout of the board.
pub fn widget_sandbox_slot_scene(
    board: &WidgetSandboxBoard,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let edge = palette.border;
    let title = sandbox_readout(board, WidgetSandboxTextKind::Title);
    bsn! {
            Node {
                position_type: PositionType::Absolute,
                right: px(space::S16),
                top: px(space::S16),
                width: px(360.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S6),
                padding: UiRect::all(Val::Px(space::S12)),
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
            }
            BackgroundColor({ palette.surface })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            WidgetSandboxSlotRoot
            Children [
                Text(title) TextRole(Role::Heading) WidgetSandboxText(WidgetSandboxTextKind::Title)
                --
                Text(String::new()) TextRole(Role::Caption) WidgetSandboxText(WidgetSandboxTextKind::Identity)
                --
                Text(String::new()) TextRole(Role::Caption) WidgetSandboxText(WidgetSandboxTextKind::Slot)
                --
                Text(String::new()) TextRole(Role::Caption) WidgetSandboxText(WidgetSandboxTextKind::Quota)
                --
                Text(String::new()) TextRole(Role::Caption) WidgetSandboxText(WidgetSandboxTextKind::Grant)
                --
                Text(String::new()) TextRole(Role::Caption) WidgetSandboxText(WidgetSandboxTextKind::Status)
            ]
    }
}

/// Render one restampable readout line from the sandbox board.
pub fn sandbox_readout(board: &WidgetSandboxBoard, kind: WidgetSandboxTextKind) -> String {
    match kind {
        WidgetSandboxTextKind::Title => "Widget Sandbox".to_owned(),
        WidgetSandboxTextKind::Identity => match board.first_attached() {
            Some(widget) => format!(
                "{} · v{} · {}",
                widget.widget_id, widget.version, widget.plugin_name
            ),
            None => "No widget attached".to_owned(),
        },
        WidgetSandboxTextKind::Slot => match board.first_attached() {
            Some(widget) => {
                let slot = widget.render_slot();
                format!(
                    "slot {} · {}x{} · r{}",
                    slot.slot_id(),
                    slot.width(),
                    slot.height(),
                    slot.revision()
                )
            }
            None => "slot —".to_owned(),
        },
        WidgetSandboxTextKind::Quota => match board.first_attached() {
            Some(widget) => format!(
                "memory {}/{} B · frame {}",
                widget.memory_used(),
                widget.quota.max_memory_bytes,
                widget.frame_index()
            ),
            None => "quota —".to_owned(),
        },
        WidgetSandboxTextKind::Grant => match board.first_attached() {
            Some(widget) => {
                let grants: Vec<&str> = widget
                    .granted_permissions()
                    .iter()
                    .map(|permission| host_capability_label(permission.host_capability()))
                    .collect();
                if grants.is_empty() {
                    "permissions: none".to_owned()
                } else {
                    format!("permissions: {}", grants.join(", "))
                }
            }
            None => "permissions —".to_owned(),
        },
        WidgetSandboxTextKind::Status => attach_status(board),
    }
}

fn attach_status(board: &WidgetSandboxBoard) -> String {
    if let Some((capability, _)) = board.last_denial {
        return format!("denied capability: {}", host_capability_label(capability));
    }
    match board.last_outcome() {
        Some(WidgetAttachOutcome::Attached { .. }) => "attached".to_owned(),
        Some(WidgetAttachOutcome::Rejected(fault)) => format!("rejected: {fault:?}"),
        Some(WidgetAttachOutcome::Incompatible(AbiNegotiation::VersionMismatch {
            required,
            host,
        })) => format!("ABI mismatch: widget {required:?} host {host:?}"),
        Some(WidgetAttachOutcome::Incompatible(AbiNegotiation::MissingCapabilities {
            missing,
        })) => format!(
            "missing capabilities: {}",
            widget_capability_labels(*missing)
        ),
        Some(WidgetAttachOutcome::Incompatible(AbiNegotiation::Compatible { .. })) => {
            "attached".to_owned()
        }
        None => "idle".to_owned(),
    }
}

fn widget_capability_labels(capabilities: HostCapabilities) -> String {
    let labels: Vec<&str> = capabilities.iter().map(widget_capability_label).collect();
    if labels.is_empty() {
        "none".to_owned()
    } else {
        labels.join(", ")
    }
}

fn widget_capability_label(capability: WidgetCapability) -> &'static str {
    match capability {
        WidgetCapability::GpuShaders => "gpu-shaders",
        WidgetCapability::TouchInput => "touch-input",
        WidgetCapability::Gamepad => "gamepad",
        WidgetCapability::ImeComposition => "ime",
        WidgetCapability::MultiWindow => "multi-window",
        WidgetCapability::HapticFeedback => "haptics",
    }
}

/// Human-readable name of a host capability surfaced by the sandbox.
pub fn host_capability_label(capability: HostCapability) -> &'static str {
    match capability {
        HostCapability::ReadTrafficStats => "read traffic stats",
        HostCapability::ReadNodeList => "read node list",
        HostCapability::SwitchProxyNode => "switch proxy node",
        HostCapability::ManageProfiles => "manage profiles",
        HostCapability::ExecuteNetworkDiagnostics => "run network diagnostics",
        HostCapability::NetworkRead => "network read",
    }
}

/// Product assembly: ABI host, board, visibility latch and the slot systems.
pub struct WidgetSandboxPlugin;

impl Plugin for WidgetSandboxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WidgetHostAbi>();
        app.init_resource::<WidgetSandboxBoard>();
        app.init_resource::<WidgetSandboxVisibility>();
        app.init_resource::<WidgetSandboxMounted>();
        app.add_observer(on_attach_sandbox_widget);
        app.add_observer(on_authorize_sandbox_capability);
        app.add_systems(
            Update,
            (sync_widget_sandbox_slot, refresh_widget_sandbox_slot).chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::app::App;
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::image::Image;
    use bevy::scene::ScenePlugin;
    use infiltrator_bevy_widgets::sandbox::WidgetPermission;
    use infiltrator_bevy_widgets::theme::Theme;

    fn manifest() -> WidgetManifest {
        WidgetManifest::new("community.speedtest", "Community Speed Test")
            .with_permission(WidgetPermission::ReadTrafficStats)
    }

    fn attach_default(board: &mut WidgetSandboxBoard) -> WidgetAttachOutcome {
        board.attach(
            WidgetHostAbi::default(),
            &manifest(),
            WidgetAbiRequirement::for_current_abi(),
        )
    }

    #[test]
    fn compatible_attach_assigns_an_isolated_render_slot() {
        let mut board = WidgetSandboxBoard::new();
        let slot = match attach_default(&mut board) {
            WidgetAttachOutcome::Attached { slot, .. } => slot,
            other => panic!("expected attach, got {other:?}"),
        };
        assert!(slot.is_valid());
        assert_eq!(slot.slot_id(), 0);
        assert_eq!(slot.width(), WIDGET_SLOT_WIDTH);
        assert_eq!(board.attached_count(), 1);
        let widget = board.first_attached().expect("attached widget");
        assert_eq!(widget.render_slot(), slot);
        assert!(
            widget.authorize(HostCapability::ReadTrafficStats).is_ok(),
            "the bound manifest grants exactly its declared permission"
        );
    }

    #[test]
    fn version_mismatch_is_typed_and_never_attaches() {
        let mut board = WidgetSandboxBoard::new();
        let host = WidgetHostAbi {
            version: WIDGET_ABI_VERSION,
            capabilities: HostCapabilities::all_desktop(),
        };
        let outcome = board.attach(host, &manifest(), WidgetAbiRequirement::new((1, 0, 0)));
        assert_eq!(
            outcome,
            WidgetAttachOutcome::Incompatible(AbiNegotiation::VersionMismatch {
                required: (1, 0, 0),
                host: WIDGET_ABI_VERSION,
            })
        );
        assert_eq!(
            board.attached_count(),
            0,
            "a mismatch is not a silent success"
        );
    }

    #[test]
    fn missing_capability_is_typed_and_never_attaches() {
        let mut board = WidgetSandboxBoard::new();
        let host = WidgetHostAbi {
            version: WIDGET_ABI_VERSION,
            capabilities: HostCapabilities::mobile_default(),
        };
        let requirement =
            WidgetAbiRequirement::for_current_abi().with_capability(WidgetCapability::MultiWindow);
        match board.attach(host, &manifest(), requirement) {
            WidgetAttachOutcome::Incompatible(AbiNegotiation::MissingCapabilities { missing }) => {
                assert!(missing.has(WidgetCapability::MultiWindow));
            }
            other => panic!("expected missing capability, got {other:?}"),
        }
        assert_eq!(board.attached_count(), 0);
    }

    #[test]
    fn invalid_manifest_is_rejected_typed() {
        let mut board = WidgetSandboxBoard::new();
        let mut manifest = manifest();
        manifest.min_bevy_version = "0.19.1".to_owned();
        assert_eq!(
            board.attach(
                WidgetHostAbi::default(),
                &manifest,
                WidgetAbiRequirement::for_current_abi()
            ),
            WidgetAttachOutcome::Rejected(SandboxFault::ManifestInvalid(
                "Incompatible Bevy version dependency"
            ))
        );
        assert_eq!(board.attached_count(), 0);
    }

    #[test]
    fn denied_capability_is_surfaced_typed() {
        let mut board = WidgetSandboxBoard::new();
        attach_default(&mut board);
        assert_eq!(
            board.authorize_capability(HostCapability::ManageProfiles),
            Err(SandboxFault::CapabilityDenied(
                HostCapability::ManageProfiles
            ))
        );
        assert_eq!(
            board.denied_capability(),
            Some(HostCapability::ManageProfiles)
        );
        assert_eq!(board.denial_count(), 1);
        assert!(
            sandbox_readout(&board, WidgetSandboxTextKind::Status).contains("manage profiles"),
            "the denial is rendered as a typed capability name"
        );
    }

    #[test]
    fn dashboard_slot_mounts_and_restamps_typed_readouts() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins((AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        let palette = UiPalette::new(&Theme::dark());
        app.insert_resource(palette);
        app.add_plugins(WidgetSandboxPlugin);
        app.world_mut().commands().trigger(AttachSandboxWidget::new(
            manifest(),
            WidgetAbiRequirement::for_current_abi(),
        ));
        app.update();
        assert!(
            app.world().resource::<WidgetSandboxVisibility>().0,
            "a successful attach reveals the dashboard slot"
        );
        app.update();

        let world = app.world_mut();
        let roots = world.query::<&WidgetSandboxSlotRoot>().iter(world).count();
        assert_eq!(roots, 1, "the dashboard slot mounts once");
        let texts: Vec<String> = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect();
        assert!(
            texts
                .iter()
                .any(|text| text.contains("community.speedtest"))
        );
        assert!(texts.iter().any(|text| text.contains("320x180")));
        assert!(texts.iter().any(|text| text.contains("read traffic stats")));
    }
}
