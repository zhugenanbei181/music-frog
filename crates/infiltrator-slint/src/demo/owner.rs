//! The demo owner's answers to the commands the capture's gestures send.
//!
//! The capture drives the real product callbacks, so the demo host has to
//! answer the shared commands the business sends. The answers are typed
//! failures of the shared contract: the surface then shows the owner's
//! refusal in the business's own panel and in the owner-visible notice, so
//! the receipt proves that the business really ran and never that a route
//! changed.

use super::fixtures;
use crate::{CommandIntent, CommandOutput, SlintAction};
use infiltrator_contract::script_export_review::{ScriptExportIdentity, ScriptExportReview};
use infiltrator_contract::snapshot_restore::{SnapshotRestoreIdentity, SnapshotRestoreReview};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_domain::profile_source::hash_document_bytes;
use infiltrator_domain::script_export_review::content_sha256;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::outbound::identity::SnapshotRevision;
use lurker_core_contract::outbound::preferences::{
    OutboundUiPreferenceIntent, OutboundUiPreferencesSnapshot, OutboundUiPreferencesValues,
};
use lurker_core_contract::page::PageData;
use lurker_core_contract::profile_document_identity::{ProfileDocumentRevision, ProfileId};

/// Answers one owner action the way the capture's business needs it.
pub fn respond(
    snapshot: &mut SurfaceSnapshot,
    action: SlintAction,
) -> Result<CommandOutput, Failure> {
    match action {
        SlintAction::Command(intent) => respond_to_intent(snapshot, &intent),
        SlintAction::Sequence(intents) => match intents.first() {
            Some(intent) => Err(refusal(intent)),
            None => Ok(CommandOutput::Unit),
        },
        _ => Ok(CommandOutput::Unit),
    }
}

/// The demo owner's answer to one business command.
///
/// The Overview card order and the outbound search query are the owner's own
/// facts, so the demo owner applies the change against the facts it holds and
/// answers with the facts it now holds. Every other command is refused with a
/// typed failure, so the receipt shows the business's own failure path and
/// never a route change.
fn respond_to_intent(
    snapshot: &mut SurfaceSnapshot,
    intent: &CommandIntent,
) -> Result<CommandOutput, Failure> {
    match intent {
        CommandIntent::MoveOverviewCard { kind, direction } => {
            let mut layout = snapshot.overview_layout.clone();
            layout.move_card(*kind, *direction);
            snapshot.overview_layout = layout;
            Ok(CommandOutput::OverviewLayoutApplied(
                snapshot.overview_layout.clone(),
            ))
        }
        CommandIntent::ResetOverviewCardOrder => {
            let mut layout = snapshot.overview_layout.clone();
            layout.reset_to_default();
            snapshot.overview_layout = layout;
            Ok(CommandOutput::OverviewLayoutApplied(
                snapshot.overview_layout.clone(),
            ))
        }
        CommandIntent::ApplyOutboundUiPreference { request } => {
            // The outbound preferences are the owner's fact, so the owner
            // applies the search query it was asked for and publishes the new
            // query: the proxies page really narrows the node list.
            match request.intent() {
                OutboundUiPreferenceIntent::SetSearchQuery(query) => {
                    apply_search_query(snapshot, query)?;
                    Ok(CommandOutput::Unit)
                }
                _ => Err(refusal(intent)),
            }
        }
        CommandIntent::LoadSnapshotDiff { .. } => {
            let diff = fixtures::yaml_ast_diff();
            snapshot.yaml_ast_diff = Some(diff.clone());
            Ok(CommandOutput::SnapshotDiffLoaded(Some(Box::new(diff))))
        }
        CommandIntent::PrepareScriptExport { draft } => {
            let mut script_export = fixtures::script_export();
            script_export.kind = draft.kind;
            script_export.profile = draft.profile.clone();
            let sha256 = content_sha256(&script_export.content);
            let review = ScriptExportReview {
                draft: draft.clone(),
                identity: ScriptExportIdentity {
                    owner: 1,
                    sequence: 1,
                    sha256,
                },
                snapshot: script_export,
            };
            Ok(CommandOutput::ScriptExportPrepared(Box::new(review)))
        }
        CommandIntent::PrepareSnapshotRestore { target } => {
            let mut diff = fixtures::yaml_ast_diff();
            diff.target_id = target.profile.clone();
            diff.source_path = Some(target.snapshot_id.clone());
            let mut workspace = fixtures::profile_workspace_identity();
            let profile_id =
                ProfileId::new(&target.profile).unwrap_or(workspace.document.profile_id);
            workspace.document.profile_id = profile_id;
            // The workbench validates the review identities against the
            // bytes, so the fixture carries the real document hashes.
            let before_yaml = "mode: global\n";
            let restored_yaml = "mode: rule\n";
            workspace.document.revision =
                ProfileDocumentRevision::new(hash_document_bytes(before_yaml))
                    .expect("hex digest is a valid revision");
            let review = SnapshotRestoreReview {
                identity: SnapshotRestoreIdentity {
                    owner: 1,
                    sequence: 1,
                    snapshot_hash: hash_document_bytes(restored_yaml),
                    workspace_identity: workspace,
                },
                target: target.clone(),
                before_yaml: before_yaml.to_string(),
                restored_yaml: restored_yaml.to_string(),
                difference: diff,
            };
            Ok(CommandOutput::SnapshotRestorePrepared(Box::new(review)))
        }
        _ => Err(refusal(intent)),
    }
}

/// Applies the search query to the published outbound preferences.
///
/// The preference is the owner's fact, so the owner publishes the new query
/// with a new revision and the surface renders the narrowed node list.
fn apply_search_query(snapshot: &mut SurfaceSnapshot, query: &str) -> Result<(), Failure> {
    let Some(preferences) = snapshot.outbound_ui_preferences.data.as_ref() else {
        return Err(Failure::unsupported("节点显示偏好尚未观测到"));
    };
    let scope = preferences.scope().clone();
    let revision = SnapshotRevision::new(preferences.revision().value() + 1);
    let values = OutboundUiPreferencesValues {
        search_query: query.to_owned(),
        favorite_outbound_ids: preferences.favorite_outbound_ids.clone(),
        group_order: preferences.group_order.clone(),
        collapsed_group_ids: preferences.collapsed_group_ids.clone(),
        sort_order: preferences.sort_order,
        health_filter: preferences.health_filter,
        layout: preferences.layout,
    };
    let applied = OutboundUiPreferencesSnapshot::new(scope, revision, values)
        .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
    snapshot.outbound_ui_preferences = PageData::ready(applied);
    Ok(())
}

/// The typed refusal the demo owner answers a business command with.
///
/// The message names the business that refused, so the receipt shows which
/// command ran.
fn refusal(intent: &CommandIntent) -> Failure {
    let message = match intent {
        CommandIntent::TestDnsLeak => "DNS 泄漏探测无法连接到已配置的解析服务器",
        CommandIntent::QueryDns { .. } => "上游解析服务器拒绝了 DNS 查询",
        CommandIntent::ClearDnsCache { .. } => "核心拒绝清理 Fake-IP 缓存",
        CommandIntent::ApplyDnsSettings { .. } => "核心拒绝了 Hosts 草稿设置",
        CommandIntent::RunScriptSandbox { .. } => "沙盒环境拒绝运行该脚本",
        CommandIntent::PrepareScriptExport { .. } => "无法生成供审核的导出工件",
        CommandIntent::LoadSnapshotDiff { .. } => "无法读取已存储的快照差异",
        CommandIntent::PrepareSnapshotRestore { .. } => "无法读取用于恢复的快照数据",
        CommandIntent::SaveSubscriptionFilter { .. } => "无法保存订阅节点过滤规则",
        CommandIntent::PreviewProfileAggregation { .. } => "无法计算配置聚合预览",
        CommandIntent::CreateAggregatedProfile { .. } => "无法创建聚合配置文件",
        CommandIntent::RunDoctorDiagnostics => "诊断任务未能连通核心服务",
        CommandIntent::BootstrapDoctor => "无法重新初始化核心引擎",
        CommandIntent::RepairDoctorIssue { .. } | CommandIntent::RepairAllDoctorIssues => {
            "核心拒绝了诊断修复请求"
        }
        CommandIntent::ResolveConflictKeepLocal | CommandIntent::ResolveConflictTakeRemote => {
            "无法解决数据同步冲突"
        }
        CommandIntent::SetAppRule { .. } => "无法应用应用分流规则",
        CommandIntent::CloseAllConnections => "核心拒绝中断所有活动连接",
        CommandIntent::CloseConnection { .. } => "核心拒绝中断该活动连接",
        CommandIntent::PrepareLogExport => "无法生成日志导出工件",
        CommandIntent::RefreshPublicIpProbe => "公网 IP探测未能连通目标服务",
        CommandIntent::StartSpeedtest { .. } => "测速任务无法连接到测速目标",
        CommandIntent::SetRuntimeProbeSettings { .. } => "无法将探测配置写入核心",
        CommandIntent::ApplyOutboundUiPreference { .. } => "无法应用节点显示偏好设置",
        CommandIntent::CommitRuleList { .. } => "无法提交分流规则列表",
        CommandIntent::CreateNamedOverride { .. } => "无法创建命名 Override",
        CommandIntent::RefreshRuleProviders => "无法刷新规则集提供源",
        CommandIntent::ResetRuleHitCounters { .. } => "无法重置规则命中计数器",
        CommandIntent::SetLanSecurity { .. } => "无法应用局域网安全策略",
        CommandIntent::RepairPortConflicts => "无法自动修复端口冲突",
        CommandIntent::RunPrivilegedNetworkRegression => "无法执行特权网络回归测试",
        CommandIntent::CheckVpnAuthorization => "无法检查 VPN 授权状态",
        CommandIntent::SetRoutingIntent { .. } => "无法应用路由意图",
        CommandIntent::GetRoutingIntent => "无法回读当前路由意图",
        CommandIntent::UpdateAllSubscriptions => "无法更新所有订阅",
        CommandIntent::RestartCore => "无法重启核心服务",
        CommandIntent::UpdateSetting { .. } => "无法保存设置项",
        _ => "所有者拒绝了该命令",
    };
    Failure::new(ErrorCode::Internal, message, true)
}
