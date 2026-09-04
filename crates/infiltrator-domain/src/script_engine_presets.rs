//! Built-in script preset catalog.

use super::{HookStage, ScriptEngine, ScriptPreset};

impl ScriptEngine {
    pub fn builtin_presets() -> Vec<ScriptPreset> {
        vec![
            ScriptPreset {
                id: "remove-ads",
                name: "Remove Ads & Tracking Rules (移除广告与垃圾节点)",
                description: "自动剔除名称中包含'官网'、'剩余'、'广告'、'重置'等非可用节点，并清理广告拦截规则",
                stage: HookStage::PreMerge,
                script_code: "function main(config, profile) {\n    filter_nodes_by_regex(config, \"官网|剩余|到期|重置|广告|notice|traffic|reset\", true);\n    remove_rules(config, \"REJECT|AdBlock|adblock|advertising\");\n    console.log(\"Ads and tracking nodes/rules filtered successfully\");\n    return config;\n}",
            },
            ScriptPreset {
                id: "auto-country-groups",
                name: "Auto Country Proxy Groups (按国家/地区自动分组)",
                description: "根据节点名称中的国家标识（香港/日本/美国/新加坡）自动创建策略组与测速组",
                stage: HookStage::PreMerge,
                script_code: "function main(config, profile) {\n    auto_country_groups(config);\n    console.log(\"Country proxy groups generated successfully\");\n    return config;\n}",
            },
            ScriptPreset {
                id: "streaming-groups",
                name: "Streaming Services Dedicated Groups (流媒体独立分流分组)",
                description: "为 Netflix、Disney+、YouTube、OpenAI 自动创建独立分流策略组与专属路由规则",
                stage: HookStage::PostMerge,
                script_code: "function main(config, profile) {\n    streaming_groups(config);\n    console.log(\"Streaming service groups and rules configured\");\n    return config;\n}",
            },
            ScriptPreset {
                id: "direct-china",
                name: "Direct China LAN & GeoIP (国内直连与私网分流)",
                description: "注入局域网私有网段与 GeoIP CN 中国直连规则，降低网络延迟并杜绝 DNS 泄露",
                stage: HookStage::PostMerge,
                script_code: "function main(config, profile) {\n    direct_china(config);\n    console.log(\"Direct China and LAN routing applied\");\n    return config;\n}",
            },
        ]
    }

    pub fn find_preset(id: &str) -> Option<ScriptPreset> {
        Self::builtin_presets().into_iter().find(|p| p.id == id)
    }
}
