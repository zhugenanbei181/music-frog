use crate::state::AppState;
use crate::tray::tray_events_subscription;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::types::runtime::{RuntimeStatus, RuntimeStreamKind, RuntimeStreamState};
use futures_util::StreamExt;
use iced::futures::stream::BoxStream;
use iced::{Subscription, stream, window};
use infiltrator_application::system_proxy_application::SystemProxyApplication;
use infiltrator_ports::host_runtime::HostRuntime;
use infiltrator_ports::runtime_gateway::RuntimeStreamEvent;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
struct RuntimeStreamInput {
    identity: usize,
    generation: u64,
    gateway: Arc<dyn HostRuntime>,
    log_level: String,
}

#[derive(Clone)]
struct SystemProxyWatchdogInput {
    identity: usize,
    application: SystemProxyApplication,
}

impl Hash for SystemProxyWatchdogInput {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.identity.hash(state);
    }
}

impl Hash for RuntimeStreamInput {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.identity.hash(state);
        self.generation.hash(state);
        self.log_level.hash(state);
    }
}

/// Start the three controller streams as one declarative subscription. The
/// identity is tied to the runtime Arc and CoreApplication generation, so a
/// stopped/rebuilt core cancels all old receivers before a new one starts.
pub(crate) fn runtime_streams_subscription(
    runtime: &Arc<dyn HostRuntime>,
    log_level: &str,
) -> Subscription<Message> {
    let input = RuntimeStreamInput {
        identity: Arc::as_ptr(runtime) as *const () as usize,
        generation: runtime.generation(),
        gateway: runtime.clone(),
        log_level: log_level.to_string(),
    };
    Subscription::run_with(input, build_runtime_stream)
}

fn build_runtime_stream(input: &RuntimeStreamInput) -> BoxStream<'static, Message> {
    let input = input.clone();
    let channel = stream::channel(
        256,
        move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
            loop {
                let _ = output.try_send(stream_state(
                    RuntimeStreamKind::Logs,
                    input.generation,
                    RuntimeStreamState::Connecting,
                ));
                let _ = output.try_send(stream_state(
                    RuntimeStreamKind::Traffic,
                    input.generation,
                    RuntimeStreamState::Connecting,
                ));
                let _ = output.try_send(stream_state(
                    RuntimeStreamKind::Connections,
                    input.generation,
                    RuntimeStreamState::Connecting,
                ));

                let logs = input
                    .gateway
                    .stream_logs(Some(input.log_level.clone()))
                    .await;
                let traffic = input.gateway.stream_traffic().await;
                let connections = input.gateway.stream_connections().await;

                let (mut logs, mut traffic, mut connections) = match (logs, traffic, connections) {
                    (Ok(logs), Ok(traffic), Ok(connections)) => (logs, traffic, connections),
                    (logs, traffic, connections) => {
                        if let Err(error) = logs {
                            let _ = output.try_send(stream_state(
                                RuntimeStreamKind::Logs,
                                input.generation,
                                RuntimeStreamState::Failed(error.to_string()),
                            ));
                        }
                        if let Err(error) = traffic {
                            let _ = output.try_send(stream_state(
                                RuntimeStreamKind::Traffic,
                                input.generation,
                                RuntimeStreamState::Failed(error.to_string()),
                            ));
                        }
                        if let Err(error) = connections {
                            let _ = output.try_send(stream_state(
                                RuntimeStreamKind::Connections,
                                input.generation,
                                RuntimeStreamState::Failed(error.to_string()),
                            ));
                        }
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        continue;
                    }
                };

                loop {
                    tokio::select! {
                        item = logs.next() => match item {
                            Some(RuntimeStreamEvent::Item(line)) => {
                                if output.try_send(Message::RuntimeStreamLogReceived(input.generation, line)).is_err() { return; }
                            }
                            Some(RuntimeStreamEvent::Connecting) => {
                                if output.try_send(stream_state(RuntimeStreamKind::Logs, input.generation, RuntimeStreamState::Connecting)).is_err() { return; }
                            }
                            Some(RuntimeStreamEvent::Connected) => {
                                if output.try_send(stream_state(RuntimeStreamKind::Logs, input.generation, RuntimeStreamState::Connected)).is_err() { return; }
                            }
                            Some(RuntimeStreamEvent::Reconnecting(error)) | Some(RuntimeStreamEvent::Failed(error)) => {
                                if output.try_send(stream_state(RuntimeStreamKind::Logs, input.generation, RuntimeStreamState::Failed(error))).is_err() { return; }
                                if output.try_send(stream_state(RuntimeStreamKind::Logs, input.generation, RuntimeStreamState::Reconnecting)).is_err() { return; }
                            }
                            None => break,
                        },
                        item = traffic.next() => match item {
                            Some(RuntimeStreamEvent::Item(data)) => {
                                if output
                                    .try_send(Message::RuntimeStreamTrafficReceived(
                                        input.generation,
                                        data,
                                    ))
                                    .is_err()
                                {
                                    return;
                                }
                            }
                            Some(RuntimeStreamEvent::Connecting) => {
                                if output.try_send(stream_state(RuntimeStreamKind::Traffic, input.generation, RuntimeStreamState::Connecting)).is_err() { return; }
                            }
                            Some(RuntimeStreamEvent::Connected) => {
                                if output.try_send(stream_state(RuntimeStreamKind::Traffic, input.generation, RuntimeStreamState::Connected)).is_err() { return; }
                            }
                            Some(RuntimeStreamEvent::Reconnecting(error)) | Some(RuntimeStreamEvent::Failed(error)) => {
                                if output.try_send(stream_state(RuntimeStreamKind::Traffic, input.generation, RuntimeStreamState::Failed(error))).is_err() { return; }
                                if output.try_send(stream_state(RuntimeStreamKind::Traffic, input.generation, RuntimeStreamState::Reconnecting)).is_err() { return; }
                            }
                            None => break,
                        },
                        item = connections.next() => match item {
                            Some(RuntimeStreamEvent::Item(snapshot)) => {
                                if output
                                    .try_send(Message::RuntimeStreamConnectionsReceived(
                                        input.generation,
                                        snapshot,
                                    ))
                                    .is_err()
                                {
                                    return;
                                }
                            }
                            Some(RuntimeStreamEvent::Connecting) => {
                                if output.try_send(stream_state(RuntimeStreamKind::Connections, input.generation, RuntimeStreamState::Connecting)).is_err() { return; }
                            }
                            Some(RuntimeStreamEvent::Connected) => {
                                if output.try_send(stream_state(RuntimeStreamKind::Connections, input.generation, RuntimeStreamState::Connected)).is_err() { return; }
                            }
                            Some(RuntimeStreamEvent::Reconnecting(error)) | Some(RuntimeStreamEvent::Failed(error)) => {
                                if output.try_send(stream_state(RuntimeStreamKind::Connections, input.generation, RuntimeStreamState::Failed(error))).is_err() { return; }
                                if output.try_send(stream_state(RuntimeStreamKind::Connections, input.generation, RuntimeStreamState::Reconnecting)).is_err() { return; }
                            }
                            None => break,
                        },
                    }
                }

                for kind in [
                    RuntimeStreamKind::Logs,
                    RuntimeStreamKind::Traffic,
                    RuntimeStreamKind::Connections,
                ] {
                    let _ = output.try_send(stream_state(
                        kind,
                        input.generation,
                        RuntimeStreamState::Reconnecting,
                    ));
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        },
    );
    Box::pin(channel)
}

pub(crate) fn system_proxy_watchdog_subscription(
    application: &Option<SystemProxyApplication>,
) -> Subscription<Message> {
    let Some(application) = application else {
        return Subscription::none();
    };
    let input = SystemProxyWatchdogInput {
        identity: application.identity(),
        application: application.clone(),
    };
    Subscription::run_with(input, build_system_proxy_watchdog)
}

fn build_system_proxy_watchdog(input: &SystemProxyWatchdogInput) -> BoxStream<'static, Message> {
    let application = input.application.clone();
    let channel = stream::channel(
        16,
        move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3));
            loop {
                interval.tick().await;
                let snapshot = application.snapshot().await;
                if output
                    .try_send(Message::SystemProxyReconciled(snapshot))
                    .is_err()
                {
                    return;
                }
            }
        },
    );
    Box::pin(channel)
}

fn stream_state(kind: RuntimeStreamKind, generation: u64, state: RuntimeStreamState) -> Message {
    Message::RuntimeStreamStateChanged {
        kind,
        generation,
        state,
    }
}

impl AppState {
    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![];

        // 1. Tray Events (neutral channel; only subscribed when a tray is live)
        if let Some(rx) = &self.shell.tray_events {
            subs.push(tray_events_subscription(rx));
        }

        // 1b. Admin host commands (context -> app bridge for the Web UI)
        if let Some(rx) = &self.shell.admin_commands {
            subs.push(crate::admin_server::admin_commands_subscription(rx));
        }

        // 1c. Shared application surface snapshots. The desktop/mobile
        // composition owns the pump; Iced only drains typed messages.
        if let Some(bridge) = &self.surface_bridge {
            subs.push(bridge.subscription());
        }

        // System proxy ownership is host-side state rather than core stream
        // state. Keep a three-second reconciliation loop even when Mihomo is
        // stopped so a stale desktop proxy can be restored safely.
        if !self.shell.demo {
            subs.push(system_proxy_watchdog_subscription(
                &self.runtime.system_proxy_application,
            ));
        }

        // 2. Scheduled subscription auto-update checks
        subs.push(Subscription::run(|| {
            stream::channel(
                100,
                |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                    let mut sub_interval = tokio::time::interval(Duration::from_secs(900));
                    loop {
                        sub_interval.tick().await;
                        let _ = output.try_send(Message::TickSubUpdate);
                    }
                },
            )
        }));

        // 3. WebDAV scheduling is independent of the core lifecycle: profile
        // files can be synchronized while mihomo is stopped. The persisted
        // interval is part of the subscription identity, so changing it
        // replaces the old timer declaratively.
        if self.profile.webdav_enabled {
            let interval_secs = self
                .profile
                .webdav_sync_interval_mins
                .trim()
                .parse::<u64>()
                .ok()
                .filter(|minutes| *minutes > 0)
                .unwrap_or(60)
                .saturating_mul(60);
            subs.push(Subscription::run_with(interval_secs, |seconds: &u64| {
                let interval = Duration::from_secs(*seconds);
                stream::channel(
                    100,
                    move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                        let mut sync_interval = tokio::time::interval(interval);
                        loop {
                            sync_interval.tick().await;
                            let _ = output.try_send(Message::TickWebDavSync);
                        }
                    },
                )
            }));
        }
        if let Some(runtime) = self.runtime.runtime.as_ref()
            && matches!(self.runtime.status, RuntimeStatus::Running)
        {
            subs.push(runtime_streams_subscription(runtime, &self.diag.log_level));
        }
        if self.runtime.runtime.is_some()
            && self.shell.current_route == Route::Runtime
            && self.runtime.runtime_auto_refresh
            && matches!(self.runtime.status, RuntimeStatus::Running)
        {
            subs.push(Subscription::run(|| {
                stream::channel(
                    100,
                    |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                        let mut refresh_interval =
                            tokio::time::interval(Duration::from_millis(2000));
                        loop {
                            refresh_interval.tick().await;
                            let _ = output.try_send(Message::TickRuntimeRefresh);
                        }
                    },
                )
            }));
        }

        // 4. 全局键盘订阅：把每一次按下原样转成 `KeyboardChord`，由
        // `update` 对照共享 `ShortcutRegistry` 解析（Command Palette
        // Ctrl+K / Cmd+K、系统代理、TUN、Mini HUD、主题循环），捕获模式下
        // 则作为新的绑定组合键。订阅是纯转发，因此不需要捕获状态。
        subs.push(iced::event::listen_with(|event, _status, _window| {
            let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) =
                event
            else {
                return None;
            };
            let key_name = match &key {
                iced::keyboard::Key::Character(character) => character.to_string(),
                iced::keyboard::Key::Named(named) => format!("{named:?}"),
                _ => return None,
            };
            Some(Message::KeyboardChord {
                key: key_name,
                modifiers: infiltrator_contract::shortcuts::KeyModifiers {
                    ctrl: modifiers.control(),
                    shift: modifiers.shift(),
                    alt: modifiers.alt(),
                    meta: modifiers.command(),
                },
            })
        }));

        // 4a. 系统外观订阅：`system` 偏好下即时跟随 OS 明暗切换。
        subs.push(
            iced::system::theme_changes()
                .map(|mode| Message::SystemThemeChanged(matches!(mode, iced::theme::Mode::Dark))),
        );

        // 4b. 窗口尺寸订阅：唯一驱动共享 4 阶响应式投影的来源。窗口拖拽
        // 只影响本地布局，但阶的判定必须走 shared contract，两端一致。
        subs.push(
            window::resize_events()
                .map(|(_id, size)| Message::WindowResized(size.width, size.height)),
        );

        // 4c. 窗口焦点订阅（DUAL-15-08）：唯一驱动共享渲染调步的宿主事实。
        subs.push(window::events().filter_map(|(_id, event)| match event {
            window::Event::Focused => Some(Message::WindowFocusChanged(true)),
            window::Event::Unfocused => Some(Message::WindowFocusChanged(false)),
            _ => None,
        }));

        // 5. 高性能动画订阅：只有正在转场时才开启帧回调；帧率由共享
        //    `RenderCadence` 决定（前台跟随真实帧信号，后台 2 FPS）。
        if self.shell.transition.start_time.is_some()
            || (self.shell.current_route == Route::Overview
                && self.runtime.traffic_topology.is_flowing())
        {
            subs.push(frame_cadence_subscription(
                infiltrator_contract::cadence::RenderCadence::from_focused(
                    self.shell.window_focused,
                ),
            ));
        }

        Subscription::batch(subs)
    }
}

/// The animation frame tick at the shared cadence (DUAL-15-08): the foreground
/// keeps Iced's real frame signal, the background is a 2 FPS timer, and a
/// suspended host schedules no frames at all.
pub(crate) fn frame_cadence_subscription(
    cadence: infiltrator_contract::cadence::RenderCadence,
) -> Subscription<Message> {
    use infiltrator_contract::cadence::RenderCadence;
    match cadence {
        RenderCadence::Active => window::frames().map(Message::TickFrame),
        RenderCadence::Background => Subscription::run_with(cadence, |cadence: &RenderCadence| {
            let cadence = *cadence;
            stream::channel(
                4,
                move |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                    let Some(interval) = cadence.frame_interval() else {
                        return;
                    };
                    let mut ticker = tokio::time::interval(interval);
                    loop {
                        ticker.tick().await;
                        if output
                            .try_send(Message::TickFrame(std::time::Instant::now()))
                            .is_err()
                        {
                            break;
                        }
                    }
                },
            )
        }),
        RenderCadence::Suspended => Subscription::none(),
    }
}
