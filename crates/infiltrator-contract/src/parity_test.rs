use super::FeatureId;
use std::collections::BTreeSet;

#[test]
fn declarations_cover_the_typed_registry_without_duplicate_or_missing_cells() {
    let ids: BTreeSet<_> = FeatureId::ALL
        .iter()
        .map(|feature| feature.spec().id)
        .collect();
    assert_eq!(
        ids.len(),
        FeatureId::ALL.len(),
        "duplicate feature identity"
    );
    let manifest = include_str!("../../../scripts/parity/cross_surface_manifest.tsv");
    let evidence = include_str!("../../../scripts/parity/feature_evidence.tsv");
    let mut surfaces = BTreeSet::new();
    for line in manifest.lines().skip(1) {
        let cells: Vec<_> = line.split('\t').collect();
        assert_eq!(cells.len(), 6, "malformed manifest row: {line}");
        assert!(ids.contains(cells[0]), "unknown feature: {}", cells[0]);
        assert!(["iced", "bevy"].contains(&cells[1]));
        assert!(
            surfaces.insert((cells[0], cells[1])),
            "duplicate surface cell"
        );
    }
    let expected_surfaces: BTreeSet<_> = ids
        .iter()
        .flat_map(|id| [(*id, "iced"), (*id, "bevy")])
        .collect();
    assert_eq!(surfaces, expected_surfaces);
    let mut levels = BTreeSet::new();
    for line in evidence.lines().skip(1) {
        let cells: Vec<_> = line.split('\t').collect();
        assert_eq!(cells.len(), 7, "malformed evidence row: {line}");
        assert!(
            levels.insert((cells[0], cells[1], cells[2])),
            "duplicate evidence cell"
        );
    }
    let expected_levels: BTreeSet<_> = surfaces
        .iter()
        .flat_map(|(id, surface)| [(*id, *surface, "contract"), (*id, *surface, "scenario")])
        .collect();
    assert_eq!(levels, expected_levels);
}
