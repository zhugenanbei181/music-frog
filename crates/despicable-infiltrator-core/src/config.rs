use anyhow::anyhow;
use yaml_rust2::YamlLoader;

pub fn validate_yaml(content: &str) -> anyhow::Result<()> {
    if content.trim().is_empty() {
        return Err(anyhow!("配置内容不能为空"));
    }
    YamlLoader::load_from_str(content)
        .map(|_| ())
        .map_err(|_| anyhow!("配置内容不是有效的 YAML"))
}
