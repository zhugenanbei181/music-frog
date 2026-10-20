# 中性 crate 用例展厅 (Examples Gallery)

> 层级：L2

`examples/` 是七个中性 Lurker crate 的公开门面：四套独立极简用例分别展示终端、Slint、Iced 与 Bevy 表面如何消费同一份中性契约与呈现内核。第三方开发者读一份用例即可知道该 crate 提供什么、该表面该写什么。

## 四套用例

| 用例 | 目录 | 表面 | 展示的中性能力 |
| --- | --- | --- | --- |
| `minimal-cli` | `examples/minimal-cli` | 终端 | 契约身份、生命周期枚举与呈现内核的横幅判决投影 |
| `minimal-slint` | `examples/minimal-slint` | Slint 1.18 | 同一个 typed 生命周期经 Slint 属性投影到界面 |
| `minimal-iced` | `examples/minimal-iced` | Iced 0.14 | 同一个 typed 生命周期经 Iced `Message` 投影到界面 |
| `minimal-bevy` | `examples/minimal-bevy` | Bevy 0.20 | 同一个 typed 生命周期经 Bevy ECS 资源投影到界面 |

每套用例都是独立包，各自带一个 `[[example]]` 目标，名字与目录一致：

```bash
cd examples/minimal-cli && cargo run --example minimal-cli
cd examples/minimal-iced && cargo run --example minimal-iced
```

从仓库根目录运行时用 `-p` 指定包：

```bash
cargo run -p lurker-example-minimal-cli --example minimal-cli
```

## 用例纪律

- **行数预算**：每份用例源文件不超过 200 行，由 `scripts/quality/examples-guard.py` 强制。
- **依赖白名单**：用例只能依赖已发布的中性 crate 与它演示的那一个 UI 工具链；Slint 的构建期伴生 `slint-build` 除外。工具链的版本与特性必须与同名产品模板逐字一致，避免出现第二份版本事实。
- **不发布**：每个用例包 `publish = false`，它们是仓库内的展品，不是 crates.io 的包。
- **唯一真源**：表面只渲染中性层报告的事实，不在本地发明第二份业务状态；阶段一律用 typed enum 表达，不用布尔标志。

## 门禁

| 门禁 | 入口 | 作用 |
| --- | --- | --- |
| 结构门 | `python3 scripts/quality/examples-guard.py --mode structure` | 离线审计目录、目标、依赖白名单、行数预算与 CI 编译门接线 |
| CI 编译门 | `python3 scripts/quality/examples-guard.py --mode compile` | 逐个 `cargo build --example`，任何一个用例编译失败即失败 |
| 无头冒烟 | `python3 scripts/quality/examples-guard.py --mode smoke` | 运行唯一可在无显示环境运行的终端用例并核对输出 |

结构门已接入 `scripts/quality/check-structure.sh`。CI 编译门的工作流草案见 `scripts/ci/ecosystem-gate.yml`；该草案尚未进入 `.github/workflows/`，原因与登记方式写在草案头部注释里。
