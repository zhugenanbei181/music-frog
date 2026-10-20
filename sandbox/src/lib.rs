//! Headless sandbox and chaos fault injection for the neutral Lurker runtime.
//!
//! The sandbox is the integration surface of the neutral runtime: it runs
//! the real [`lurker_engine_runtime::facade::LurkerRuntime`] over adapters
//! that inject deterministic faults, and it reads the typed answers back.
//!
//! The four fault classes of the matrix are
//!
//! * a network flap of the controller endpoint,
//! * the sudden death of the kernel process during the start,
//! * the contention of the cross-process write fence,
//! * a corrupted profile document on disk.
//!
//! Three properties hold for every scenario, and every assertion is about
//! the typed failure and the typed recovery:
//!
//! * **Fail closed.** An injected fault never produces a success that the
//!   runtime did not observe. The runtime publishes the exact failure phase
//!   and refuses to run again from a failed state.
//! * **Reclaimed resources.** The owner lease is released after its exact
//!   terminal, the write fence is acquirable again after its holder is gone,
//!   and every started process is stopped again.
//! * **Self healing.** A host that observes the typed failure can build a
//!   fresh runtime over a fresh process and reach the running state again.
//!
//! The sandbox never uses a random number generator and never derives an
//! outcome from a wall-clock measurement.
//!
//! # Example
//!
//! ```
//! use lurker_core_contract::identity::EngineId;
//! use lurker_sandbox::fault::Fault;
//! use lurker_sandbox::ports::ChaosPhase;
//! use lurker_sandbox::runtime_chaos::Sandbox;
//!
//! let sandbox = Sandbox::new(EngineId::mihomo());
//! sandbox.inject(Fault::NetworkFlap, u32::MAX);
//! let runtime = sandbox.runtime().expect("the sandbox runtime builds");
//! assert_eq!(sandbox.process().phase(), ChaosPhase::Stopped);
//! drop(runtime);
//! ```

#![warn(missing_docs)]

pub mod fault;
pub mod ports;
pub mod runtime_chaos;
pub mod store_chaos;
