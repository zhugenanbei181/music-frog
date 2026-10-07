use infiltrator_contract::parity::FeatureId;
fn main() {
    let specs: Vec<_> = FeatureId::ALL
        .iter()
        .map(|feature| feature.spec())
        .collect();
    println!(
        "{}",
        serde_json::to_string(&specs).expect("serializable feature registry")
    );
}
