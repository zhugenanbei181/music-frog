//! Application seam for orchestrating the Overview headless behavior and regression matrix.

use infiltrator_contract::overview_matrix::OverviewRegressionMatrixReport;

/// Application facade providing on-demand verification of the Overview capability suite.
#[derive(Clone, Copy, Debug, Default)]
pub struct OverviewMatrixApplication;

impl OverviewMatrixApplication {
    pub fn execute(&self) -> OverviewRegressionMatrixReport {
        OverviewRegressionMatrixReport::run_deterministic_matrix()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_application_execution() {
        let app = OverviewMatrixApplication;
        let report = app.execute();
        assert!(report.is_all_passed());
        assert_eq!(report.total_scenarios, 14);
    }
}
