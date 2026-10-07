//! Tracked inspector probes preserve failures and never reopen a dismissed panel.
use crate::state::AppState;
use crate::types::message::Message;
use iced::Task;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};

#[derive(Clone, Debug)]
pub struct PendingProxyProbe {
    pub name: String,
    pub token: u64,
}
#[derive(Default)]
pub struct ProxyProbeState {
    pub pending: Option<PendingProxyProbe>,
    pub failure: Option<Failure>,
    next_token: u64,
}
impl ProxyProbeState {
    pub fn dismiss(&mut self) {
        self.pending = None;
        self.failure = None;
    }
}
impl AppState {
    pub(crate) fn probe_inspected_proxy(&mut self) -> Task<Message> {
        if self.runtime.inspection_probe.pending.is_some() {
            return Task::none();
        }
        let Some(name) = self.runtime.inspecting_proxy.clone() else {
            return Task::none();
        };
        if self.surface.latest().is_some() && !self.runtime.inspection_read.can_probe() {
            return Task::none();
        }
        if !self
            .proxy_inspection(&name)
            .is_some_and(|detail| detail.can_probe)
        {
            return Task::none();
        }
        let application = self.commands.clone();
        let runtime = self.runtime.runtime.clone();
        if application.is_none() && runtime.is_none() {
            self.runtime.inspection_probe.failure = Some(Failure::new(
                ErrorCode::NotReady,
                "proxy probe service is not composed",
                true,
            ));
            return Task::none();
        }
        self.runtime.inspection_probe.next_token =
            self.runtime.inspection_probe.next_token.wrapping_add(1);
        let token = self.runtime.inspection_probe.next_token;
        self.runtime.inspection_probe.pending = Some(PendingProxyProbe {
            name: name.clone(),
            token,
        });
        self.runtime.inspection_probe.failure = None;
        let node = name.clone();
        let probe_options = self.applied_probe_options().ok();
        let (url, timeout_ms) = match probe_options {
            Some(ref opt) => (Some(opt.test_url.clone()), Some(opt.timeout_ms)),
            None => (None, None),
        };
        let fallback_options = if application.is_none() {
            match self.applied_probe_options() {
                Ok(options) => Some(options),
                Err(failure) => {
                    self.runtime.inspection_probe.pending = None;
                    self.runtime.inspection_probe.failure = Some(failure);
                    return Task::none();
                }
            }
        } else {
            None
        };
        Task::perform(
            async move {
                if let Some(application) = application {
                    match (application
                        .execute(CommandIntent::TestNodeDelay {
                            node,
                            url,
                            timeout_ms,
                        })
                        .await)
                        .into_unit()
                    {
                        Ok(()) => Ok(()),
                        Err(failure) => Err(failure),
                    }
                } else if let Some(runtime) = runtime {
                    runtime
                        .test_delay(
                            &node,
                            &fallback_options
                                .as_ref()
                                .expect("legacy parameters validated")
                                .test_url,
                            fallback_options
                                .as_ref()
                                .expect("legacy parameters validated")
                                .timeout_ms,
                        )
                        .await
                        .map(|_| ())
                        .map_err(Failure::from)
                } else {
                    unreachable!("probe composition checked before admission")
                }
            },
            move |result| Message::ProxyInspectionProbed {
                name: name.clone(),
                token,
                result,
            },
        )
    }
    pub(crate) fn finish_inspection_probe(
        &mut self,
        name: String,
        token: u64,
        result: Result<(), Failure>,
    ) -> Task<Message> {
        if !self
            .runtime
            .inspection_probe
            .pending
            .as_ref()
            .is_some_and(|pending| pending.name == name && pending.token == token)
        {
            return Task::none();
        }
        self.runtime.inspection_probe.pending = None;
        if self.runtime.inspecting_proxy.as_deref() != Some(name.as_str()) {
            return Task::none();
        }
        self.runtime.inspection_probe.failure = result.err();
        if self.runtime.inspection_probe.failure.is_some() {
            Task::none()
        } else {
            Task::done(Message::LoadProxies)
        }
    }
}
