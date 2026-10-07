# infiltrator-bevy-ui

MusicFrog 的 Bevy UI 产品，与 Iced 同权、独立发行。它以 ECS、`bsn!` 场景树和 Observer 构建原生界面，通过共享 application 消费投影并提交 typed 命令。控件由 `infiltrator-bevy-widgets` 提供。

当前包含 Overview、Proxies、Profiles、Rules、Connections、Logs、DNS、Doctor、App Routing、Sync 和 Settings 路由；生产数据源由宿主 composition 注入，演示模式使用隔离 fixture。连接详情使用抽屉，测速明细使用模态，关闭全部连接使用可取消的独立确认模态。主题、响应式布局和无障碍节点由控件层组织。

原生入口的通用卡片使用 GPU Squircle 材质，尺寸与主题变化保留控件和材质身份；宿主可以选择平面回退。静态图表保留纹理，只有完整样本、显示参数或主题变化时刷新。三维地理拓扑与 GPU 波形增强仍在实施计划中，尚未作为已完成能力交付。

现有路由或 headless 测试通过不代表全部业务交互已经对齐。复杂操作面与标准/紧凑视口的视觉证据仍在逐场景验收，当前缺口以双端平权审计为准。

此 crate 属于根 workspace。验证入口是根目录的 `bash scripts/test-bevy.sh`。`cargo run -p infiltrator-bevy-ui` 加载桌面真实宿主、配置与完整页面数据源；停止的内核通过用户操作启动。初始化失败显示失败状态。隔离演示使用 `INFILTRATOR_DEMO=1 cargo run -p infiltrator-bevy-ui`。Android 宿主与打包能力单独验证，不能从桌面运行结果外推。
