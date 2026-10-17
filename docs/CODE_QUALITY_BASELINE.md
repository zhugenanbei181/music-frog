# 代码质量底线

> 层级：L2

本文只保留全仓规则。分层边界见 [ARCHITECTURE.md](ARCHITECTURE.md)；Bevy 与 Android 专项要求分别见其本地文档。

## 源码与职责

- Rust、Kotlin 生产代码、测试、示例和构建辅助源码均按单文件 **800 行**预算执行；不压行、不提高阈值、不加白名单或用源码拼接绕过。
- 导入使用定义模块；禁止长路径代替导入、命名导入别名、`pub use` 转发和纯类型转发。
- 外部能力、缺失观测、失败、取消和不支持使用明确类型表达；不得用默认值、空数据或静默 no-op 冒充成功。
- 文案由 `infiltrator-shared` 统一维护。UI 视图、状态和异步结果均使用共享本地化资源。
- 所有多阶段操作都必须以一个显式 enum/sealed 状态表达互斥状态和合法迁移，并由穷尽匹配或纯 reducer 转移。未观测、进行中、取消、超时、成功、失败、过期、待清理等必须使用各自的 typed variant；禁止用多个 bool/Option 组合藏出隐式状态机、用 action 字符串表示命令，或用 bool 参数/返回值表示请求种类、准入和操作结果。
- `bool` 只表示真实二值事实（例如配置开关）或只用于呈现的派生投影；派生投影不得授权副作用。`Option`/nullable 只表示单一、明确的可选数据，不能同时代表未读、失败、不支持、未确认等状态。操作阶段转移不得依靠散布的 `if/else` 和早退标志；输入校验、范围检查等局部条件仍可使用条件分支。
- 已完成切片的精确退役 public symbol 与 CLI surface 登记于 `scripts/quality/retired-surfaces.json`，由结构门扫描生产入口与权威产品文档，并要求每项指向替代行为测试；历史 TODO 与归档不作为产品接口，但也不能被误用作当前状态。登记项的作用域只能指向仓库跟踪（git tracked）的权威源码；生成物与 gitignore 的构建产物不构成产品表面，禁止作为作用域。
- **公开命名先验证可用性**：任何新增公开 crate 名、产品名或发布标识，落笔前必须先查 crates.io（及目标平台）是否已被占用；只接受可用的空闲名，占用则换名再开工。禁止先写代码或文档、发布时才发现重名。中性框架 crate 统一用「潜伏者 / Lurker」品牌前缀家族。
- **架构裁决前置与反修补底线**：遇到任何架构未决、模型不全、生命周期二义性或读写不同步之处，必须严格执行 [ARCHITECTURE.md](ARCHITECTURE.md) 的自底向上逐层解决纪律。严禁在上层（UI、Presentation、Host 组装层）就地打补丁、增加容错垫片、增加临时布尔/状态标志位、自行推导业务事实（如通过本地比对 ID 猜测活动状态），或写空实现/TODO 注释绕行。凡包含此类上层修补的代码变更一律判定为质量不达标，必须退回底层先决后再行收口。
- **单一事实与权威读回底线**：所有受控业务状态必须由唯一的底层 Owner 拥有并通过 typed Readback 权威读回。UI 严禁在本地发明或维护第二份业务真源，严禁在未收到底层 Owner 权威读回前单方面断定或呈现成功状态。

## Kotlin / Android 代码质量

- Android lifecycle、service owner、permission flow、Binder protocol、repository 与 ViewModel 状态转换同样遵循上述约束；行为测试必须覆盖每个可达 variant 的合法转移与拒绝分支。
- BANDROID-044 未完成前，不声明 Android Lint、ktlint、Detekt 或 Kotlin 架构守卫已接入；两 flavor 的格式/静态规则和正反例通过后，才将其加为强制 CI 门。专项要求登记在本地 TODO。

## Slint UI 代码质量与模块化规范

- `.slint` 声明代码单文件预算受 `scripts/quality/slint-syntax-guard.py` 严格管控（最大非空非注释代码行预算 $\le 600$ 行），严禁无限制堆叠单文件；
- 必须遵循模块化、组件化架构：主题（`theme.slint`）、数据模型（`models.slint`）、交互基础控件（`base_widgets.slint`）与业务卡片（`cards.slint`）必须拆分子模块独立导入，禁止在页面级或主窗口内大杂烩内联；
- 严禁硬编码未归一化的原始状态颜色（如 `#0B667F`、`#9A6810` 等），所有视觉和语义色彩必须经由 `LumenTheme` 的语义化属性统一输出；
- 移动端控件（按钮、点击卡片、开关、导航项等）在 `compact-layout` 模式下必须遵守 $\ge 48\text{px} \times \text{LumenUiScale.factor}$ 的最小触控热区底线。

## 验证

- 质量门只在 `scripts/quality/check-structure.sh` 汇总；禁止通过删除断言、放宽门槛、消音错误或复制第二套检查获得通过。

Bevy 的 ECS、shader 和资源生命周期规则见 [Bevy UI 章程](../crates/infiltrator-bevy-ui/docs/BEVY_UI_FRONTEND.md)；Android host 与 IPC 边界见 [Android 章程](../android/docs/BEVY_ANDROID_PRODUCT.md)。
