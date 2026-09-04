use crate::state::AppState;
use crate::tray::tray_events_subscription;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::types::runtime::{RuntimeStatus, RuntimeStreamKind, RuntimeStreamState};
use iced::futures::stream::BoxStream;
use iced::{Subscription, stream, window};
use infiltrator_ports::runtime_gateway::{ManagedRuntime, RuntimeGateway, RuntimeStreamEvent};
use futures_util::StreamExt;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
struct RuntimeStreamInput {
    identity: usize,
    generation: u64,
    gateway: Arc<dyn RuntimeGateway>,
    log_level: String,
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
pub(crate) fn runtime_streams_subscription<R>(runtime: &Arc<R>, log_level: &str) -> Subscription<Message>
where
    R: ManagedRuntime + 'static,
{
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
                                        data.into(),
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
                                        snapshot.into(),
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

        // 4. 全局键盘热键订阅（Command Palette Ctrl+K / Cmd+K 与 ESC）
        subs.push(iced::event::listen_with(|event, _status, _window| {
            if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key, modifiers, ..
            }) = event
            {
                if (modifiers.control() || modifiers.command())
                    && (key == iced::keyboard::Key::Character("k".into())
                        || key == iced::keyboard::Key::Character("K".into()))
                {
                    return Some(Message::ToggleCommandPalette);
                }
                if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) {
                    return Some(Message::CloseCommandPalette);
                }
            }
            None
        }));

        // 5. 高性能动画订阅：只有正在转场时才开启帧回调
        if self.shell.transition.start_time.is_some() {
            subs.push(window::frames().map(Message::TickFrame));
        }

        Subscription::batch(subs)
    }
}
