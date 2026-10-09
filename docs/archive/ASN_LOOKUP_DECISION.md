# ASN / GeoIP 归属透视：证据与决策（DUAL-13-05）

> 结论先行：**不引入客户端 MMDB 解析器**。v1.19.18 内核的 `/connections` metadata 已原生携带
> `destinationGeoIP` 与 `destinationIPASN`，客户端只做诚实三态投影（见台账 `DUAL-13-05`，已从
> `planned` 推进为 `shared-ready`）。本地 `geoip.metadb` / `ASN.mmdb` 的格式与候选 crate 证据保留在
> 本文，作为内核 API 失效时的迁移方案。

## 1. 调查环境

| 事实 | 值 |
| --- | --- |
| 仓库内核 | `vendor/mihomo.exe` = **Mihomo Meta v1.19.18** windows amd64, go1.25.5（`vendor/mihomo.exe -v`，经 wine 读取），sha256 `3dbf9a49398ab5608c285b9175d55ba4fb06fb914c69f59222024baff7f354ed` |
| 本应用配置目录 | `~/.config/mihomo-rs/`（`crates/mihomo-platform/src/paths.rs:44`）——本机为**空目录**，没有任何 geo 文件 |
| 本机另一客户端 | `~/.config/mihomo-party/work/`（mihomo-party，**不是本应用**）留有真实 geo 数据库，用作格式证据 |

## 2. 关键实证：内核 `/connections` 原生暴露 ASN 与 GeoIP

台账原判断「mihomo `/connections` exposes no ASN field」对 v1.19.18 已过时：

1. 二进制证据：`strings vendor/mihomo.exe` 含 `json:"destinationIPASN"` 与
   `json:"destinationGeoIP"`，同段类型元数据为 `DstGeoIP`/`DstIPASN`/`SrcPort`/`InPort`/
   `SpecialProxy`/`SpecialRules`（`constant.Metadata` 字段组）。
2. 源码证据（v1.19.18 `constant/metadata.go`）：
   ```go
   SrcGeoIP  []string `json:"sourceGeoIP"`      // nil if never queried, [] if no result
   DstGeoIP  []string `json:"destinationGeoIP"` // nil if never queried, [] if no result
   SrcIPASN  string   `json:"sourceIPASN"`
   DstIPASN  string   `json:"destinationIPASN"`
   ```
   `/connections` 由 `statistic.DefaultManager.Snapshot()` 直接序列化该 metadata
   （`hub/route/connections.go`）。
3. 写入时机（`rules/common/geoip.go`、`rules/common/ipasn.go`）：GEOIP / IP-ASN 规则求值时写入；
   `destinationIPASN = asn + " " + aso`（`component/mmdb/reader.go: LookupASN`，未命中返回 `""`/`""`）。
   因此客户端可区分三态：
   - `destinationGeoIP`: `null` = 内核对本次连接从未求值；`[]` = 已求值无记录；数组 = 真实国家码。
   - `destinationIPASN`: `""` = 未求值；`" "`（纯空白）= 已求值无 ASN 记录；其他 = 内核原始串，
     例如 `15169 Google LLC`（内核**不加 `AS` 前缀**）。
4. 诚实边界：这两个字段是**规则求值产物**，不是对每个连接的全量 ASN 库查询。没有 GEOIP/IP-ASN 规则
   时客户端必须显示「内核未对本次连接求值」，不能自行读取数据库补齐。

已落地（DUAL-13-05）：`mihomo-api` → domain `ConnectionMetadata` → contract
`ConnectionSnapshot.destination_geo_ip/destination_ip_asn` → 领域
`connection_view::destination_geo_fact`/`destination_asn_fact` 三态归约 → Iced drawer 与 Bevy
`ConnDrawerFieldKind::KernelAsn/KernelGeo`，标注真实来源 `/connections ...`。

## 3. 本地 geo 文件格式证据（本机 mihomo-party 目录）

| 文件 | 大小 | sha256 | 容器 | 元数据 |
| --- | --- | --- | --- | --- |
| `ASN.mmdb` | 12,103,050 | `7dcc428e…08f5950` | **MaxMind DB** | `database_type=GeoLite2-ASN`，`ip_version=6`，`record_size=24`，`node_count=1,579,735` |
| `geoip.metadb` | 8,510,129 | `1b237ecc…c29619e` | **MaxMind DB 容器** | `database_type=Meta-geoip0`，`ip_version=6`，`record_size=24`，`node_count=1,417,116` |
| `country.mmdb` | 8,510,129 | `6eef2fed…d714720` | 同上（Meta-geoip0 副本） | 与 `geoip.metadb` 同构，build_epoch 不同 |
| `geoip.dat` | 16,907,278 | `f3370cf3…7e02d72` | v2ray protobuf（`0a aa 29` → `GeoIPList`） | 非 MMDB |
| `geosite.dat` | 4,244,510 | `03e53e16…7b304a4` | v2ray protobuf（`0a 38` → `GeoSiteList`） | 非 MMDB |

判定依据：
- MMDB 尾部元数据标记 `ab cd ef 4d 61 78 4d 69 6e 64 2e 63 6f 6d`（`\xab\xcd\xefMaxMind.com`）在
  `ASN.mmdb`、`geoip.metadb`、`country.mmdb` 均存在；`record_size=24`、`ip_version=6` 与文件大小吻合
  （`node_count * record_size * 2 / 8` 正好是数据段起点）。
- `geoip.metadb` 数据段仅 7,248 字节，内容是 2 字母国家码（`th/jp/cn/us/...`）与服务标签
  （`google/cloudflare/netflix/facebook/twitter/fastly/private`）；全文件 **0 次**
  `autonomous_system_number`。
- `ASN.mmdb` 数据段含 `autonomous_system_number`、`autonomous_system_organization` 与真实组织名
  （`Google LLC`、`Cloudflare, Inc.`、`Amazon.com, Inc.` 等）。
- 结论：应用自己 `ensure_geoip_database` 下载的 `geoip.metadb` **只含国家/服务码，不含 ASN**；
  ASN 只在 `ASN.mmdb`（内核按需下载，`~/.config/mihomo-rs` 目前并不存在该文件）。
- 注意容器 ≠ schema：`Meta-geoip0` 用标准 MMDB 容器承载 MetaCubeX 自定义记录（mihomo
  `component/mmdb/reader.go` 对 `typeMetaV0` 做特殊解码），公开的 MaxMind schema 并不覆盖它。

## 4. 客户端自行下载/读取的事实

- 应用只保证 `geoip.metadb`：`crates/infiltrator-desktop/src/runtime.rs:872-878`（
  `GEOIP_URL`/mirrors/`GEOIP_MIN_SIZE`），`ensure_geoip_database` 写到 `<config dir>/geoip.metadb`；
  仅当 profile 文本含 `GEOIP` 时才触发；**没有任何代码路径写 `ASN.mmdb`**。
- `infiltrator-domain/src/geo_lookup_cache.rs` 的 `GeoLookupCache`/`GeoInfo` 没有生产调用方
  （仅模块声明与自带测试），是未接线死代码；`infiltrator-iced` 的 `GeoDataStatus`
  （`geoip_size_bytes` 等）也从未被填充，卡片渲染 `—`/unknown。
- 内核侧：`component/mmdb/mmdb.go` 用 `oschwald/maxminddb-golang` 读 `C.Path.MMDB()`
  （即 `geoip.metadb`）与 `C.Path.ASN()`（`ASN.mmdb`）；ASN 数据库由内核
  `geodata.UpdateGeoDatabases()` 下载（二进制字符串：
  `https://github.com/MetaCubeX/meta-rules-dat/releases/download/latest/GeoLite2-ASN.mmdb`、
  `Can't find ASN.mmdb, start download`、`Download ASN.mmdb finish`），并可由应用现有
  `RuntimeGateway::upgrade_geo()`（`POST /upgrade/geo`）触发。

## 5. 候选依赖审计（`Cargo.lock` 为准）

| crate | 在 lockfile | 判断 |
| --- | --- | --- |
| `maxminddb` | **无** | 读 MMDB 的成熟实现；引入 = 新 workspace 依赖 + 联网取包，本项不引入 |
| `ipnetwork` | 无 | 与 MMDB 解码无关 |
| `prost` / `prost-derive` | **无**（仅本机 cargo cache 有 0.13.5 `.crate` 包，未进 lockfile） | 读 `.dat` protobuf 需新依赖 + 生成 `GeoIPList` schema |
| `ipnet` | 有（传递依赖） | 仅 CIDR 运算，不能解码 MMDB/MetaDB |
| `md5`、`sha2`、`hex` | 有 | 只能做哈希，不能解码 |

即：自行实现完整 MMDB 读取器不需要新依赖，但那是本项目当前没有、也无法在 CI 内用真实数据库验证的
解析器（fixture 只能自造，存在自证风险），且会与内核重复同一份离线查询。**故不做客户端解析。**

## 6. 决策

1. **采用内核 API 事实**：`/connections` 的 `destinationGeoIP`/`destinationIPASN` 是单一事实源，
   无新依赖、无格式风险、无伪造空间。
2. **不引入 `maxminddb`、不实现私有 MMDB 解析器、不读 `geoip.metadb`/`ASN.mmdb`**。
3. **不伪造**：客户端不补 `AS` 前缀、不把 `Meta-geoip0` 当国家库、不在未求值时猜归属；服务端/两
   端 UI 均保留三态（未求值/无记录/内核原值）。

## 7. 迁移方案（仅当内核不再暴露这两个字段时）

1. 新增 workspace 依赖 `maxminddb`（crates.io，MaxMind DB 读取器；锁版本后 vendor 进构建面）。
2. 只读内核目录下的真实文件，按存在性降级：
   - `<config dir>/ASN.mmdb` → `autonomous_system_number` + `autonomous_system_organization`
     （`GeoLite2-ASN` / `DBIP-ASN-Lite (compat=GeoLite2-ASN)`）。
   - `<config dir>/geoip.metadb` → 需自定义 `Meta-geoip0` record 解码（string 或 `[]string`），
     不能套用 MaxMind 的 `geoip2Country` schema。
3. 数据库供给：应用侧不新增下载器；沿用内核 `POST /upgrade/geo`（现成 `RuntimeGateway::upgrade_geo`）
   或用户在 profile 依赖的规则下让内核按需下载 `ASN.mmdb`。
4. 测试策略：fixture 必须能证明与真实格式一致——用真实 `ASN.mmdb` 的子集做 `#[ignore]` 校验测试，
   CI 内用受控生成的 MMDB fixture；禁止只测自写 reader 与自写 writer 的互证。
5. 性能：沿用 `ProviderFileFingerprint` 的未变输入捷径（size + mtime 未变不重算），并对 MMDB 使用
   内存映射或按需读取，避免每次 UI tick 全量加载。

## 8. 非声明

- 本文不声称本应用已具备离线 ASN 查询；13-05 的两端事实来自**运行中的内核**。
- 本文不声称 `geoip.metadb` 可被 MaxMind 生态直接消费（`Meta-geoip0` 是 MetaCubeX 自定义 schema）。
- 本文不把 `/connections` 字段当作全量 ASN 库：它只在 GEOIP/IP-ASN 规则求值时出现。
