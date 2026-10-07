use crate::commands::ProxyAction;
use crate::context::Runtime;
use crate::output::{print_info, print_success, print_table};
use infiltrator_application::proxy_application::ProxyNode;
use infiltrator_domain::proxy::ProxyGroup;

pub(crate) async fn handle(action: ProxyAction) -> anyhow::Result<()> {
    let runtime = Runtime::detect().await?;
    let application = runtime.proxy_application().await?;
    match action {
        ProxyAction::List => {
            let nodes = application
                .list_nodes()
                .await
                .map_err(|failure| anyhow::anyhow!(failure.message))?;
            if nodes.is_empty() {
                print_info("No proxy nodes found");
            } else {
                let rows: Vec<Vec<String>> = nodes.iter().map(node_row).collect();
                print_table(&["Name", "Type", "Delay", "Alive"], &rows);
            }
        }
        ProxyAction::Groups => {
            let groups = application
                .list_groups()
                .await
                .map_err(|failure| anyhow::anyhow!(failure.message))?;
            if groups.is_empty() {
                print_info("No proxy groups found");
            } else {
                let rows: Vec<Vec<String>> = groups.iter().map(group_row).collect();
                print_table(&["Name", "Current", "Members"], &rows);
            }
        }
        ProxyAction::Switch { group, proxy } => {
            application
                .switch(&group, &proxy)
                .await
                .map_err(|failure| anyhow::anyhow!(failure.message))?;
            print_success(&format!("Group '{group}' switched to '{proxy}'"));
        }
        ProxyAction::Test {
            name,
            url,
            timeout_ms,
        } => {
            let delay = application
                .test_delay(&name, &url, timeout_ms)
                .await
                .map_err(|failure| anyhow::anyhow!(failure.message))?;
            print_success(&format!("{name}: {delay} ms"));
        }
        ProxyAction::Current { group } => {
            let current = application
                .current(&group)
                .await
                .map_err(|failure| anyhow::anyhow!(failure.message))?;
            println!("group: {group}");
            println!("current: {current}");
        }
    }
    Ok(())
}

fn node_row(node: &ProxyNode) -> Vec<String> {
    vec![
        node.name.clone(),
        node.proxy_type.clone(),
        node.delay
            .map(|delay| format!("{delay} ms"))
            .unwrap_or_else(|| "-".to_string()),
        match node.alive {
            Some(true) => "yes",
            Some(false) => "no",
            None => "unknown",
        }
        .to_string(),
    ]
}

fn group_row(group: &ProxyGroup) -> Vec<String> {
    vec![
        group.name.clone(),
        group.now.clone(),
        group.all.len().to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::{group_row, node_row};
    use infiltrator_application::proxy_application::ProxyNode;
    use infiltrator_domain::proxy::ProxyGroup;

    fn sample_node() -> ProxyNode {
        ProxyNode {
            name: "HK-01".to_string(),
            proxy_type: "Shadowsocks".to_string(),
            udp: Some(true),
            history: vec![],
            delay: Some(120),
            alive: Some(true),
        }
    }

    #[test]
    fn node_row_formats_delay_and_alive() {
        let row = node_row(&sample_node());
        assert_eq!(row[0], "HK-01");
        assert_eq!(row[2], "120 ms");
        assert_eq!(row[3], "yes");

        let mut dead = sample_node();
        dead.delay = None;
        dead.alive = Some(false);
        let row = node_row(&dead);
        assert_eq!(row[2], "-");
        assert_eq!(row[3], "no");
        dead.alive = None;
        assert_eq!(node_row(&dead)[3], "unknown");
    }

    #[test]
    fn group_row_lists_current_and_member_count() {
        let group = ProxyGroup {
            name: "GLOBAL".to_string(),
            now: "HK-01".to_string(),
            all: vec!["HK-01".to_string(), "US-01".to_string()],
            history: vec![],
        };
        let row = group_row(&group);
        assert_eq!(row[0], "GLOBAL");
        assert_eq!(row[1], "HK-01");
        assert_eq!(row[2], "2");
    }
}
