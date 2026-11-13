//! Canonical Connections view keys and their zh-CN copy.
//!
//! The dot-separated presentation IDs are the canonical message IDs; the
//! underscore keys are aliases consumed by the existing flat-key catalog
//! scanners. Both forms resolve to one copy entry here.

use std::borrow::Cow;

pub(crate) const CONNECTION_VIEW_COPY_KEYS: [&str; 27] = [
    "connection.column.status",
    "connection.column.establish_time",
    "connection.column.type",
    "connection.column.host",
    "connection.column.sniff_host",
    "connection.column.process",
    "connection.column.process_path",
    "connection.column.rule",
    "connection.column.proxy_chain",
    "connection.column.source_ip",
    "connection.column.source_port",
    "connection.column.destination_ip",
    "connection.column.destination_port",
    "connection.column.inbound_ip",
    "connection.column.inbound_port",
    "connection.column.upload_speed",
    "connection.column.download_speed",
    "connection.column.upload",
    "connection.column.download",
    "connection.column.dscp",
    "connection.column.remote_destination",
    "connection.column.dns_mode",
    "connection.column.duration",
    "connection.view_mode.list",
    "connection.view_mode.table",
    "connection.view.reset_columns",
    "connection.view.title",
];

pub(crate) const CONNECTION_VIEW_COPY_KEY_ALIASES: [&str; 27] = [
    "connection_column_status",
    "connection_column_establish_time",
    "connection_column_type",
    "connection_column_host",
    "connection_column_sniff_host",
    "connection_column_process",
    "connection_column_process_path",
    "connection_column_rule",
    "connection_column_proxy_chain",
    "connection_column_source_ip",
    "connection_column_source_port",
    "connection_column_destination_ip",
    "connection_column_destination_port",
    "connection_column_inbound_ip",
    "connection_column_inbound_port",
    "connection_column_upload_speed",
    "connection_column_download_speed",
    "connection_column_upload",
    "connection_column_download",
    "connection_column_dscp",
    "connection_column_remote_destination",
    "connection_column_dns_mode",
    "connection_column_duration",
    "connection_view_mode_list",
    "connection_view_mode_table",
    "connection_view_reset_columns",
    "connection_view_title",
];

pub(crate) fn connection_view_copy_alias(key: &str) -> Option<&'static str> {
    CONNECTION_VIEW_COPY_KEYS
        .iter()
        .position(|candidate| *candidate == key)
        .map(|index| CONNECTION_VIEW_COPY_KEY_ALIASES[index])
}

pub(super) fn translate_connection_view_zh(key: &str) -> Cow<'static, str> {
    let alias = match connection_view_copy_alias(key) {
        Some(alias) => alias,
        None => key,
    };
    translate_connection_view_flat_zh(alias)
}

fn translate_connection_view_flat_zh(key: &str) -> Cow<'static, str> {
    match key {
        "connection_column_status" => "状态".into(),
        "connection_column_establish_time" => "建立时间".into(),
        "connection_column_type" => "类型".into(),
        "connection_column_host" => "主机".into(),
        "connection_column_sniff_host" => "嗅探主机".into(),
        "connection_column_process" => "进程".into(),
        "connection_column_process_path" => "进程路径".into(),
        "connection_column_rule" => "规则".into(),
        "connection_column_proxy_chain" => "代理链".into(),
        "connection_column_source_ip" => "源 IP".into(),
        "connection_column_source_port" => "源端口".into(),
        "connection_column_destination_ip" => "目标 IP".into(),
        "connection_column_destination_port" => "目标端口".into(),
        "connection_column_inbound_ip" => "入站 IP".into(),
        "connection_column_inbound_port" => "入站端口".into(),
        "connection_column_upload_speed" => "上传速率".into(),
        "connection_column_download_speed" => "下载速率".into(),
        "connection_column_upload" => "上传".into(),
        "connection_column_download" => "下载".into(),
        "connection_column_dscp" => "DSCP".into(),
        "connection_column_remote_destination" => "远端目标".into(),
        "connection_column_dns_mode" => "DNS 模式".into(),
        "connection_column_duration" => "持续时间".into(),
        "connection_view_mode_list" => "列表视图".into(),
        "connection_view_mode_table" => "表格视图".into(),
        "connection_view_reset_columns" => "重置列".into(),
        "connection_view_title" => "连接视图".into(),
        _ => key.to_owned().into(),
    }
}
