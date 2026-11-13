//! The TUN stack selector's shared vocabulary (one owner for every surface).
//!
//! The contract owns the catalog and whether each entry may be applied; this
//! module adds the two facts a control needs on top of it: which entries a
//! surface may offer, and which shared-copy key names each one. A surface
//! therefore cannot offer a value the core would refuse, cannot spell the
//! same stack two ways, and cannot rebuild the vocabulary in its own file.

use infiltrator_contract::tun::{TunStack, TunStackChoice};
use std::collections::HashSet;

/// The entries a TUN stack selector offers, in the contract's own order.
pub const TUN_STACK_CHOICES: [TunStackChoice; 4] = TunStack::choices();

/// The shared-copy key that names one stack.
pub const fn tun_stack_copy_key(stack: TunStack) -> &'static str {
    match stack {
        TunStack::Gvisor => "tun_stack_gvisor",
        TunStack::System => "tun_stack_system",
        TunStack::Mixed => "tun_stack_mixed",
        TunStack::Lwip => "settings_lwip_reference",
    }
}

/// The entries a surface may offer: the ones the core can apply.
pub fn selectable_tun_stack_choices() -> Vec<TunStackChoice> {
    TunStack::selectable_choices().to_vec()
}

/// The entries a surface may offer, as the wire names it sends back.
pub fn selectable_tun_stack_names() -> Vec<&'static str> {
    TunStack::SELECTABLE_WIRE_NAMES.to_vec()
}

/// The shared-copy key of the entry at a selector's index.
pub fn tun_stack_copy_key_at(index: usize) -> &'static str {
    let choices = TUN_STACK_CHOICES;
    tun_stack_copy_key(choices[index.min(choices.len() - 1)].stack)
}

/// The index a wire name occupies in the shared catalog.
pub fn tun_stack_index(wire_name: &str) -> Option<usize> {
    TunStack::choice_index_for_wire(wire_name)
}

/// The index a wire name occupies among the entries a surface may offer.
pub fn selectable_tun_stack_index(wire_name: &str) -> Option<usize> {
    let wanted = wire_name.trim();
    selectable_tun_stack_names()
        .into_iter()
        .position(|choice| choice == wanted)
}

/// The wire name at a selector's index.
pub fn selectable_tun_stack_wire_name_at(index: usize) -> &'static str {
    TunStack::selectable_wire_name_at(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_offered_entries_are_exactly_the_live_ones() {
        assert_eq!(selectable_tun_stack_names(), ["gvisor", "system", "mixed"]);
    }

    #[test]
    fn every_catalog_entry_has_one_copy_key() {
        let keys: Vec<&str> = TUN_STACK_CHOICES
            .iter()
            .map(|choice| tun_stack_copy_key(choice.stack))
            .collect();
        assert_eq!(keys.len(), TUN_STACK_CHOICES.len());
        assert_eq!(
            keys.iter().collect::<HashSet<_>>().len(),
            keys.len(),
            "two entries must not share one copy key: {keys:?}"
        );
    }

    #[test]
    fn an_index_and_a_wire_name_name_the_same_entry() {
        for (index, choice) in TUN_STACK_CHOICES.iter().enumerate() {
            assert_eq!(tun_stack_index(choice.wire_name), Some(index));
            assert_eq!(tun_stack_copy_key_at(index), tun_stack_copy_key(choice.stack));
        }
        assert_eq!(tun_stack_index("unknown"), None);
    }

    #[test]
    fn the_offered_index_is_the_index_a_control_reports() {
        let names = selectable_tun_stack_names();
        for (index, name) in names.iter().enumerate() {
            assert_eq!(selectable_tun_stack_index(name), Some(index));
            assert_eq!(selectable_tun_stack_wire_name_at(index), *name);
        }
        assert_eq!(selectable_tun_stack_index("lwip"), None);
        assert_eq!(selectable_tun_stack_index("unknown"), None);
    }
}
