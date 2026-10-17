//! Injected codec between the generic machinery and a product's durable record.
//!
//! The neutral machinery never parses a product selection. It asks the codec
//! for the selection's journal revision and engine projection, and delegates
//! the whole on-disk record to [`OwnerSwitchCodec::decode`] /
//! [`OwnerSwitchCodec::encode`], which keeps product-only metadata intact and
//! therefore the persisted wire bytes unchanged.
//!
//! A codec is injected as a type parameter, so a product instantiates the
//! machinery with its own zero-sized codec and tests can instantiate the same
//! machinery with a second, test-only selection payload.

use crate::layout::ProfileStoreLayout;
use crate::owner_switch::owner::ActiveCoreOwnerState;
use lurker_core_contract::engine_switch::EngineSwitchRevision;
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;

/// The durable record layout and selection projection of one product.
pub trait OwnerSwitchCodec: Send + Sync + 'static {
    /// Product selection payload. Machinery is generic over exactly this type.
    type Selection: Clone + Eq + Send + Sync + 'static;

    /// Product durable record. The codec owns every product-only field.
    type Record: Clone + PartialEq + Send + Sync + 'static;

    /// On-disk layout that names the reference, lock and owner-proof files.
    fn layout() -> &'static ProfileStoreLayout;

    /// The neutral journal revision carried by a selection.
    fn selection_revision(selection: &Self::Selection) -> EngineSwitchRevision;

    /// The engine a selection names, when it names one. An unselected payload
    /// names none; the owner-fenced CAS uses this to decide whether a bound
    /// owner is only editing its own engine or attempting a cross-engine write.
    fn selection_engine(selection: &Self::Selection) -> Option<&EngineId>;

    /// Record for an absent durable file.
    fn pending() -> Self::Record;

    fn selection(record: &Self::Record) -> &Self::Selection;
    fn owner(record: &Self::Record) -> &ActiveCoreOwnerState<Self::Selection>;

    /// Replace the selection. Product impls may also commit product-only
    /// metadata that marks an explicit selection.
    fn set_selection(record: &mut Self::Record, selection: Self::Selection);
    fn set_owner(record: &mut Self::Record, owner: ActiveCoreOwnerState<Self::Selection>);

    fn validate(record: &Self::Record) -> Result<(), PortError>;

    /// Encode the whole persisted record, product-only fields included.
    fn encode(record: &Self::Record) -> Result<Vec<u8>, PortError>;

    /// Decode the whole persisted record, product-only fields included.
    fn decode(bytes: &[u8]) -> Result<Self::Record, PortError>;
}
