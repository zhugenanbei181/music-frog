# Bevy UI 测试质量治理与断言契约规范 (Test Governance & Zero-Tautology Policy)

本文档是 MusicFrog Infiltrator 项目 Bevy UI 体系的**测试质量刚性宪章**。
为确保工程长期演进中的高保真度与可维护性，**严禁编写任何形式的无业务断言、废话断言与报菜名断言**。

---

## 1. 核心治理原则

1. **断言即业务契约 (Assertions as Contracts)**：每一个断言必须对应一个明确的业务需求、数学不变量、状态机转移或实体生命周期特征。
2. **严禁纯存在性断言 (Zero-Tautology Rule)**：
   - 严禁单独使用 `assert!(x.is_some())`、`assert!(!vec.is_empty())`、`assert!(count > 0)` 充当测试主体；
   - 必须进一步解构具体字段，断言具体取值、类型、边界与上下文逻辑。
3. **状态转移必须前后对比 (Action-State Differential)**：
   - 测试用户交互或事件派发时，必须明确断言**操作前状态**与**操作后状态**的精确差异（如 `Before: Unchecked -> Action -> After: Checked`）；
   - 必须断言实体 ID 的不变性（`Entity ID Invariance`），确保 Bevy ECS 原地盖章、零销毁重建。
4. **数学与物理计算必须带边界断言 (Invariants & Boundary Conditions)**：
   - 贝塞尔曲线、降采样算法、TSDB 聚合、弹簧振动必须覆盖单调性、$\epsilon$ 容差、溢出截断与退化极端用例（$0$ 点、$NaN$ 防御、退化区间）。

---

## 2. 严禁的反模式与整改范例

### 反模式 1：报菜名与长度非空检查
```rust
// ❌ 严禁写法（废话断言）：
assert!(!projection.rules.is_empty());
assert!(projection.total_rules > 0);

// ✅ 标准写法（业务语义与不变量校验）：
assert_eq!(projection.rules.len(), 4);
assert_eq!(projection.rules[0].rule_type, "DOMAIN-SUFFIX");
assert_eq!(projection.rules[0].payload, "google.com");
assert_eq!(projection.rules[0].proxy, "🚀 节点选择");
assert_eq!(projection.providers[0].rule_count, 1240);
```

### 反模式 2：静态结构体字段自证
```rust
// ❌ 严禁写法：
assert_eq!(slot.view_type_id, "map_view");

// ✅ 标准写法（带行为状态流转与逻辑验证）：
let mut feed = CameraTextureFeed::default();
feed.start_stream(1920, 1080, CameraPixelFormat::Rgba8);
assert!(feed.is_streaming);
assert_eq!(feed.frame_byte_size(), 1920 * 1080 * 4);
feed.on_qr_detected("clash://install-config?url=https://sub.lan/clash.yaml");
let parsed_url = feed.parse_qr_config_url().expect("valid clash uri");
assert_eq!(parsed_url, "https://sub.lan/clash.yaml");
```

### 反模式 3：仅断言 BSN 实体数量为 1
```rust
// ❌ 严禁写法：
assert_eq!(rings.iter(world).count(), 1);

// ✅ 标准写法（校验渲染层级、Token 墨色与几何属性）：
let (ring, style, border) = rings.iter(world).next().expect("focus ring exists");
assert_eq!(style.width_px, 2.0);
assert_eq!(border.top, palette.accent);
assert_eq!(border.bottom, palette.accent);
```

---

## 3. 门禁指标与自动化审计

- 任何 PR 与新增测试必须满足：`assert_eq!` 或深度业务断言占比 **≥ 98%**；
- 静态检查工具与 CI 流水线持续扫描空泛断言，违规者自动阻断构建。
