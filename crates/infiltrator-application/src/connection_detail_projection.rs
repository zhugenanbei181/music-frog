//! Kernel attribution is folded once, preserving never queried versus no record.
use infiltrator_domain::connection_view::{
    DestinationAsnFact, DestinationGeoFact, destination_asn_fact, destination_geo_fact,
};

pub fn kernel_asn_label(raw: &str, translate: &impl Fn(&str) -> String) -> String {
    match destination_asn_fact(raw) {
        DestinationAsnFact::NotEvaluated => translate("conn_drawer_kernel_not_evaluated"),
        DestinationAsnFact::NoResult => translate("conn_drawer_kernel_no_result"),
        DestinationAsnFact::Reported(value) => value,
    }
}

pub fn kernel_geo_label(codes: Option<&[String]>, translate: &impl Fn(&str) -> String) -> String {
    match destination_geo_fact(codes) {
        DestinationGeoFact::NotEvaluated => translate("conn_drawer_kernel_not_evaluated"),
        DestinationGeoFact::NoResult => translate("conn_drawer_kernel_no_result"),
        DestinationGeoFact::Codes(codes) => codes.join(", "),
    }
}
