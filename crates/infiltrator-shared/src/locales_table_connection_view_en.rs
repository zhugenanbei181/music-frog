//! English (en-US) copy for the typed Connections view controls.

use crate::locales::locales_table::connection_view::connection_view_copy_alias;
use std::borrow::Cow;

pub(super) fn translate_connection_view_en(key: &str) -> Cow<'static, str> {
    let alias = match connection_view_copy_alias(key) {
        Some(alias) => alias,
        None => key,
    };
    match alias {
        "connection_column_status" => "Status".into(),
        "connection_column_establish_time" => "Established".into(),
        "connection_column_type" => "Type".into(),
        "connection_column_host" => "Host".into(),
        "connection_column_sniff_host" => "Sniff Host".into(),
        "connection_column_process" => "Process".into(),
        "connection_column_process_path" => "Process Path".into(),
        "connection_column_rule" => "Rule".into(),
        "connection_column_proxy_chain" => "Proxy Chain".into(),
        "connection_column_source_ip" => "Source IP".into(),
        "connection_column_source_port" => "Source Port".into(),
        "connection_column_destination_ip" => "Destination IP".into(),
        "connection_column_destination_port" => "Destination Port".into(),
        "connection_column_inbound_ip" => "Inbound IP".into(),
        "connection_column_inbound_port" => "Inbound Port".into(),
        "connection_column_upload_speed" => "Upload Speed".into(),
        "connection_column_download_speed" => "Download Speed".into(),
        "connection_column_upload" => "Upload".into(),
        "connection_column_download" => "Download".into(),
        "connection_column_dscp" => "DSCP".into(),
        "connection_column_remote_destination" => "Remote Destination".into(),
        "connection_column_dns_mode" => "DNS Mode".into(),
        "connection_view_mode_list" => "List View".into(),
        "connection_view_mode_table" => "Table View".into(),
        "connection_view_reset_columns" => "Reset Columns".into(),
        "connection_view_title" => "Connections View".into(),
        _ => key.to_owned().into(),
    }
}
