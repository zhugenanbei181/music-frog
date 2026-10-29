//! Minimal Slint desktop proxy client template (PUB-005.1).

#[allow(clippy::todo)]
mod generated_ui {
    slint::include_modules!();
}
use generated_ui::*;

use infiltrator_contract::command::CommandIntent;
use infiltrator_ports::command_sink::{CommandSink, RecordingCommandSink};
use lurker_core_contract::routing_intent::RoutingIntent;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    let main_window = AppWindow::new()?;
    let sink: Arc<dyn CommandSink> = Arc::new(RecordingCommandSink::new());

    // 1. Wire Start Core button to CommandSink
    {
        let sink = Arc::clone(&sink);
        let window_weak = main_window.as_weak();
        main_window.on_start_core(move || {
            let sink = Arc::clone(&sink);
            let window_weak = window_weak.clone();
            tokio::spawn(async move {
                if sink.submit(CommandIntent::StartCore).await.is_ok() {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(window) = window_weak.upgrade() {
                            window.set_core_status("Running".into());
                        }
                    });
                }
            });
        });
    }

    // 2. Wire Stop Core button to CommandSink
    {
        let sink = Arc::clone(&sink);
        let window_weak = main_window.as_weak();
        main_window.on_stop_core(move || {
            let sink = Arc::clone(&sink);
            let window_weak = window_weak.clone();
            tokio::spawn(async move {
                if sink.submit(CommandIntent::StopCore).await.is_ok() {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(window) = window_weak.upgrade() {
                            window.set_core_status("Stopped".into());
                        }
                    });
                }
            });
        });
    }

    // 3. Wire Routing Intent buttons to CommandSink
    {
        let sink = Arc::clone(&sink);
        let window_weak = main_window.as_weak();
        main_window.on_switch_mode(move |mode| {
            let intent = match mode.as_str() {
                "global" => RoutingIntent::RouteAllThroughProxy,
                "direct" => RoutingIntent::BypassProxy,
                _ => RoutingIntent::FollowRules,
            };
            let mode_str = format!("{intent:?}");
            let sink = Arc::clone(&sink);
            let window_weak = window_weak.clone();
            tokio::spawn(async move {
                if sink
                    .submit(CommandIntent::SetRoutingIntent { intent })
                    .await
                    .is_ok()
                {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(window) = window_weak.upgrade() {
                            window.set_routing_intent(mode_str.into());
                        }
                    });
                }
            });
        });
    }

    main_window.run()
}
