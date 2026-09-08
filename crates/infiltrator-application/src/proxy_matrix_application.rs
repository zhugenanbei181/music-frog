//! Application seam for orchestrating the Proxies & Sorting regression matrix (DUAL-04-15).

use infiltrator_contract::proxies::ProxyRegressionMatrixReport;

/// Application facade providing on-demand verification of the Group 04 Proxies capability suite.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProxyMatrixApplication;

impl ProxyMatrixApplication {
    pub fn execute(&self) -> ProxyRegressionMatrixReport {
        ProxyRegressionMatrixReport::run_deterministic_matrix()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_matrix_application_execution() {
        let app = ProxyMatrixApplication;
        let report = app.execute();
        assert!(report.is_all_passed());
        assert_eq!(report.total_scenarios, 15);
    }
}
