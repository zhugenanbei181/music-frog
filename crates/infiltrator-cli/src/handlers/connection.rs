use futures_util::StreamExt;
use infiltrator_application::connection_application::ConnectionApplication;
use infiltrator_domain::runtime::Connection;
use infiltrator_ports::runtime_gateway::RuntimeStreamEvent;

use crate::commands::ConnectionAction;
use crate::context::Runtime;
use crate::output::{self, print_info, print_success, print_table};

/// Filters accepted by `connection list`; all are substrings matched
/// case-sensitively against the connection metadata, mirroring the
/// application-level connection filtering semantics.
pub(crate) struct ListFilters {
    host: Option<String>,
    process: Option<String>,
    rule: Option<String>,
}

pub(crate) async fn handle(action: ConnectionAction) -> anyhow::Result<()> {
    let runtime = Runtime::detect().await?;
    let application = runtime.connection_application().await?;
    match action {
        ConnectionAction::List {
            host,
            process,
            rule,
            json,
        } => {
            let filters = ListFilters {
                host,
                process,
                rule,
            };
            list(&application, filters, json).await?
        }
        ConnectionAction::Stats => stats(&application).await?,
        ConnectionAction::Stream => stream(&application).await?,
        ConnectionAction::Close {
            id,
            all,
            host,
            process,
        } => close(&application, id, all, host, process).await?,
    }
    Ok(())
}

async fn list(
    application: &ConnectionApplication,
    filters: ListFilters,
    json: bool,
) -> anyhow::Result<()> {
    let connections = apply_filters(
        application
            .snapshot()
            .await
            .map_err(|failure| anyhow::anyhow!(failure.message))?
            .connections,
        &filters,
    );
    if json {
        output::print_json(&connections)?;
        return Ok(());
    }
    if connections.is_empty() {
        print_info("No active connections match the filters");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = connections.iter().map(connection_row).collect();
    print_table(&["Id", "Host", "Process", "Rule", "Up", "Down"], &rows);
    Ok(())
}

pub(crate) fn apply_filters(
    connections: Vec<Connection>,
    filters: &ListFilters,
) -> Vec<Connection> {
    connections
        .into_iter()
        .filter(|connection| {
            let host_ok = filters
                .host
                .as_deref()
                .is_none_or(|host| connection.metadata.host.contains(host));
            let process_ok = filters
                .process
                .as_deref()
                .is_none_or(|process| connection.metadata.process_path.contains(process));
            let rule_ok = filters
                .rule
                .as_deref()
                .is_none_or(|rule| connection.rule.contains(rule));
            host_ok && process_ok && rule_ok
        })
        .collect()
}

fn connection_row(connection: &Connection) -> Vec<String> {
    vec![
        short_id(&connection.id),
        connection.metadata.host.clone(),
        process_name(&connection.metadata.process_path),
        connection.rule.clone(),
        connection.upload.to_string(),
        connection.download.to_string(),
    ]
}

/// Connection ids are long UUIDs; tables only need the leading fragment.
pub(crate) fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

/// Full process paths are too wide for a table cell; the file name carries
/// the information users filter by.
pub(crate) fn process_name(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
        .to_string()
}

async fn stats(application: &ConnectionApplication) -> anyhow::Result<()> {
    let snapshot = application
        .snapshot()
        .await
        .map_err(|failure| anyhow::anyhow!(failure.message))?;
    println!("connections: {}", snapshot.connections.len());
    println!("upload total: {}", snapshot.upload_total);
    println!("download total: {}", snapshot.download_total);
    Ok(())
}

async fn stream(application: &ConnectionApplication) -> anyhow::Result<()> {
    let mut events = application
        .stream()
        .await
        .map_err(|failure| anyhow::anyhow!(failure.message))?;
    print_info("Streaming connections; press Ctrl-C to stop");
    loop {
        tokio::select! {
            event = events.next() => match event {
                Some(RuntimeStreamEvent::Item(snapshot)) => println!(
                    "connections: {}  ↑ {}  ↓ {}",
                    snapshot.connections.len(),
                    snapshot.upload_total,
                    snapshot.download_total,
                ),
                Some(RuntimeStreamEvent::Failed(error)) => return Err(anyhow::anyhow!(error)),
                Some(RuntimeStreamEvent::Reconnecting(error)) => {
                    print_info(&format!("connection stream reconnecting: {error}"));
                }
                Some(RuntimeStreamEvent::Connecting | RuntimeStreamEvent::Connected) => {}
                None => break,
            },
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    Ok(())
}

/// Exactly one selector is guaranteed by the clap ArgGroup.
async fn close(
    application: &ConnectionApplication,
    id: Option<String>,
    all: bool,
    host: Option<String>,
    process: Option<String>,
) -> anyhow::Result<()> {
    if let Some(id) = id {
        application
            .close(&id)
            .await
            .map_err(|failure| anyhow::anyhow!(failure.message))?;
        print_success(&format!("Closed connection {}", short_id(&id)));
    } else if all {
        application
            .close_all()
            .await
            .map_err(|failure| anyhow::anyhow!(failure.message))?;
        print_success("Closed all connections");
    } else if let Some(host) = host {
        let count = application
            .close_by_host(&host)
            .await
            .map_err(|failure| anyhow::anyhow!(failure.message))?;
        print_success(&format!(
            "Closed {count} connections matching host '{host}'"
        ));
    } else if let Some(process) = process {
        let count = application
            .close_by_process(&process)
            .await
            .map_err(|failure| anyhow::anyhow!(failure.message))?;
        print_success(&format!(
            "Closed {count} connections matching process '{process}'"
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "connection_test.rs"]
mod connection_test;
