use infiltrator_core::doctor::{self, DoctorEnv, DoctorFixReport, DoctorReport, DoctorStatus};

use crate::commands::DoctorAction;
use crate::handlers::EXIT_OK;
use crate::output::{self, print_info, print_table};

pub(crate) async fn handle(action: DoctorAction) -> anyhow::Result<i32> {
    match action {
        DoctorAction::Run { only, json } => {
            let report = run_report(only.as_deref()).await?;
            if json {
                output::print_json(&report)?;
            } else {
                print_report(&report);
            }
            Ok(doctor::exit_code(&report))
        }
        DoctorAction::Fix { only, json } => {
            let report = fix_report(only.as_deref()).await?;
            if json {
                output::print_json(&report)?;
            } else if report.actions.is_empty() {
                print_info("No safe doctor fixes matched the filter");
            } else {
                let rows: Vec<Vec<String>> = report
                    .actions
                    .iter()
                    .map(|action| vec![action.id.clone(), action.summary.clone()])
                    .collect();
                print_table(&["Check", "Applied Fix"], &rows);
            }
            Ok(EXIT_OK)
        }
        DoctorAction::List { json } => {
            if json {
                output::print_json(&doctor::list_checks())?;
            } else {
                let rows: Vec<Vec<String>> = doctor::list_checks()
                    .iter()
                    .map(|check| {
                        vec![
                            check.id.to_string(),
                            check.category.to_string(),
                            yes_no(check.fixable).to_string(),
                            yes_no(check.default_enabled).to_string(),
                            check.summary.to_string(),
                        ]
                    })
                    .collect();
                print_table(&["ID", "Category", "Fixable", "Default", "Summary"], &rows);
            }
            Ok(EXIT_OK)
        }
        DoctorAction::Explain { check_id } => {
            print!("{}", render_explanation(&check_id)?);
            Ok(EXIT_OK)
        }
    }
}

/// Detect the real installation and run the filtered checks. A detection
/// failure surfaces as an error so the dispatcher maps it to exit code 2.
async fn run_report(only: Option<&str>) -> anyhow::Result<DoctorReport> {
    let env = DoctorEnv::detect()?;
    Ok(doctor::run_with(&env, only).await)
}

async fn fix_report(only: Option<&str>) -> anyhow::Result<DoctorFixReport> {
    let env = DoctorEnv::detect()?;
    doctor::fix_with(&env, only).await
}

pub(crate) fn status_label(status: &DoctorStatus) -> &'static str {
    match status {
        DoctorStatus::Pass => "pass",
        DoctorStatus::Warn => "warn",
        DoctorStatus::Fail => "FAIL",
        DoctorStatus::Skip => "skip",
    }
}

pub(crate) fn report_rows(report: &DoctorReport) -> Vec<Vec<String>> {
    report
        .checks
        .iter()
        .map(|check| {
            vec![
                status_label(&check.status).to_string(),
                check.id.clone(),
                check.summary.clone(),
                check.hint.clone().unwrap_or_default(),
            ]
        })
        .collect()
}

pub(crate) fn report_summary(report: &DoctorReport) -> String {
    format!(
        "doctor: {} pass, {} warn, {} fail, {} skip",
        report.count_by_status(DoctorStatus::Pass),
        report.count_by_status(DoctorStatus::Warn),
        report.count_by_status(DoctorStatus::Fail),
        report.count_by_status(DoctorStatus::Skip),
    )
}

fn print_report(report: &DoctorReport) {
    let rows = report_rows(report);
    if rows.is_empty() {
        print_info("No doctor checks matched the filter");
        return;
    }
    print_table(&["Status", "Check", "Summary", "Hint"], &rows);
    let summary = report_summary(report);
    if report.has_failures() {
        eprintln!("{summary}");
    } else {
        println!("{summary}");
    }
}

pub(crate) fn render_explanation(check_id: &str) -> anyhow::Result<String> {
    let info = doctor::explain_check(check_id)?;
    Ok(format!(
        "{}\nid: {}\ncategory: {}\nfixable: {}\ndefault: {}\nwhy: {}\nfail means: {}\nhint: {}\n",
        info.summary,
        info.id,
        info.category,
        yes_no(info.fixable),
        yes_no(info.default_enabled),
        info.why,
        info.fail_means,
        info.hint,
    ))
}

fn yes_no(flag: bool) -> &'static str {
    if flag { "yes" } else { "no" }
}

#[cfg(test)]
#[path = "doctor_test.rs"]
mod doctor_test;
