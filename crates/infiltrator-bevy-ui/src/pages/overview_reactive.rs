//! BEVY-028: the Overview metrics projection through the reactive DAG.
//!
//! Three source signals (active connections, memory MiB, CPU tenths) feed one
//! derived load-score signal. The ECS bridge ([`ReactiveGraphPlugin`]) stages
//! only the inputs whose value actually changed, so a changed projection
//! recomputes exactly the dependent score and an idle frame does no graph work.
//!
//! The graph components ([`SignalInput`] / [`SignalOutput`]) have no `Default`,
//! so the scenes carry default-constructible markers and
//! [`bind_overview_reactive_signals`] attaches the real bindings after mount —
//! the sanctioned component-restamp route, no tree rebuild.

use crate::pages::overview::{LastOverviewProjection, OverviewReactiveSummary};
use crate::projection::OverviewProjection;
use bevy::app::{App, Update};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{With, Without};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{AlignItems, Node, Val, percent};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::reactive::{
    ReactiveDagResource, ReactiveGraphPlugin, SignalInput, SignalOutput, flush_signal_outputs,
    stage_signal_inputs,
};
use infiltrator_bevy_widgets::signal_dag::{ReactiveDag, SignalId};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// Active connection count source.
pub const SIGNAL_CONNECTIONS: SignalId = SignalId(0);
/// Core memory footprint source, in MiB.
pub const SIGNAL_MEMORY_MIB: SignalId = SignalId(1);
/// CPU utilization source, in tenths of a percent.
pub const SIGNAL_CPU_TENTHS: SignalId = SignalId(2);
/// Derived load score consumed by the page summary.
pub const SIGNAL_LOAD_SCORE: SignalId = SignalId(3);

/// Which overview metric a marked entity feeds into the graph.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverviewMetricKind {
    #[default]
    Connections,
    Memory,
    Cpu,
}

/// Default-constructible scene marker that binds an entity to a source signal.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OverviewMetricSource(pub OverviewMetricKind);

impl OverviewMetricSource {
    /// The source signal this metric writes.
    pub const fn signal(self) -> SignalId {
        match self.0 {
            OverviewMetricKind::Connections => SIGNAL_CONNECTIONS,
            OverviewMetricKind::Memory => SIGNAL_MEMORY_MIB,
            OverviewMetricKind::Cpu => SIGNAL_CPU_TENTHS,
        }
    }
}

/// The source value for active connections.
pub fn connections_value(projection: &OverviewProjection) -> i64 {
    projection.active_connections as i64
}

/// The source value for memory, in MiB (zero when unobserved).
pub fn memory_mib_value(projection: &OverviewProjection) -> i64 {
    projection.memory_bytes.unwrap_or(0) as i64 / (1024 * 1024)
}

/// The source value for CPU utilization, in tenths of a percent.
pub fn cpu_tenths_value(projection: &OverviewProjection) -> i64 {
    (projection.cpu_percent.unwrap_or(0.0) * 10.0).round() as i64
}

/// Build the deterministic metrics DAG (source ids 0..=2, derived id 3).
pub fn overview_metrics_dag() -> ReactiveDag {
    let mut dag = ReactiveDag::new();
    let _ = dag.create_signal(0);
    let _ = dag.create_signal(0);
    let _ = dag.create_signal(0);
    dag.create_computed(
        &[SIGNAL_CONNECTIONS, SIGNAL_MEMORY_MIB, SIGNAL_CPU_TENTHS],
        |values| values[0] * 1000 + values[1] + values[2],
    )
    .expect("the overview metrics DAG is acyclic");
    dag
}

/// Attach the graph bindings to the mounted markers (once each).
pub fn bind_overview_reactive_signals(
    mut commands: Commands,
    sources: Query<(Entity, &OverviewMetricSource), Without<SignalInput>>,
    outputs: Query<Entity, (With<OverviewReactiveSummary>, Without<SignalOutput>)>,
) {
    for (entity, source) in &sources {
        commands.entity(entity).insert(SignalInput {
            signal: source.signal(),
            value: 0,
        });
    }
    for entity in &outputs {
        commands.entity(entity).insert(SignalOutput {
            signal: SIGNAL_LOAD_SCORE,
            value: 0,
        });
    }
}

/// Stage the live overview metrics into the graph. Compare-and-set so an
/// unchanged metric never marks the signal dirty (idle frames stay free).
pub fn sync_overview_reactive_inputs(
    last: Res<LastOverviewProjection>,
    mut inputs: Query<&mut SignalInput>,
) {
    let Some(projection) = last.0.as_ref() else {
        return;
    };
    for mut input in &mut inputs {
        let desired = match input.signal {
            SIGNAL_CONNECTIONS => connections_value(projection),
            SIGNAL_MEMORY_MIB => memory_mib_value(projection),
            SIGNAL_CPU_TENTHS => cpu_tenths_value(projection),
            _ => continue,
        };
        if input.value != desired {
            input.value = desired;
        }
    }
}

/// Project the derived load score back onto its marked text.
pub fn restamp_overview_reactive_summary(
    mut summary: Query<(&SignalOutput, &mut Text), With<OverviewReactiveSummary>>,
) {
    for (output, mut text) in &mut summary {
        let next = output.value.to_string();
        if text.0 != next {
            text.0 = next;
        }
    }
}

/// The summary row: a caption plus the derived score text bound to the graph.
pub fn reactive_summary_scene(_palette: &UiPalette) -> impl Scene + use<> {
    bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::S8),
        }
        Children [
            Text("Load score") TextRole(Role::Caption)
            --
            Text(String::new()) OverviewReactiveSummary TextRole(Role::Mono)
        ]
    }
}

/// Installs the graph resource, the ECS bridge and the ordered staging/restamp.
pub fn register(app: &mut App) {
    app.insert_resource(ReactiveDagResource::from(overview_metrics_dag()));
    app.add_plugins(ReactiveGraphPlugin);
    app.add_systems(
        Update,
        (
            bind_overview_reactive_signals,
            sync_overview_reactive_inputs.before(stage_signal_inputs),
            restamp_overview_reactive_summary.after(flush_signal_outputs),
        ),
    );
}
