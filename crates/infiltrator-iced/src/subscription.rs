use crate::state::AppState;
use crate::types::{Message, Route, RuntimeStatus};
use iced::{Subscription, stream, window};
use muda::MenuEvent;
use std::time::Duration;
use tray_icon::TrayIconEvent;

impl AppState {
    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![];

        // 1. Tray Events
        subs.push(Subscription::run(|| {
            stream::channel(
                100,
                |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                    loop {
                        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
                            let _ = output.try_send(Message::TrayIconEvent(event));
                        }
                        if let Ok(event) = MenuEvent::receiver().try_recv() {
                            let _ = output.try_send(Message::MenuEvent(event));
                        }
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                },
            )
        }));

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

        // 3. Runtime-related background tasks
        if self.runtime.is_some() {
            subs.push(Subscription::run(|| {
                stream::channel(
                    100,
                    |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                        let mut sync_interval = tokio::time::interval(Duration::from_secs(3600));

                        loop {
                            sync_interval.tick().await;
                            let _ = output.try_send(Message::TickWebDavSync);
                        }
                    },
                )
            }));
        }
        if self.runtime.is_some()
            && self.current_route == Route::Runtime
            && self.runtime_auto_refresh
            && matches!(self.status, RuntimeStatus::Running)
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

        // 4. 高性能动画订阅：只有正在转场时才开启帧回调
        if self.transition.start_time.is_some() {
            subs.push(window::frames().map(Message::TickFrame));
        }

        Subscription::batch(subs)
    }
}
