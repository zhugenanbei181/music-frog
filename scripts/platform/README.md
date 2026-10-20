# 平台构建-安装-运行证据工具

> 层级：L3

本目录实现五大平台（Linux、Windows、macOS、Android、iOS）的构建-安装-运行证据登记册、
矩阵生成与真实运行回执登记。证据登记册是唯一真源，公共平台矩阵文档里的证据矩阵节是它的投影。

```bash
# 校验登记册与平台矩阵文档一致（守卫在 scripts/quality/platform-evidence-guard.py）
python3 scripts/quality/platform-evidence-guard.py --self-test
python3 scripts/quality/platform-evidence-guard.py --mode enforce

# 重新生成平台矩阵文档里的证据矩阵节
python3 scripts/platform/build-evidence-matrix.py
python3 scripts/platform/build-evidence-matrix.py --check

# 把一次真实安装冒烟登记进证据登记册
python3 scripts/platform/record-platform-evidence.py --platform linux \
    --run target/platform-evidence/linux/<run-id>/evidence.json
```

## 组成

| 文件 | 职责 |
| --- | --- |
| `evidence_model.py` | 证据模型：四条腿、两种状态、三种证据类型，以及矩阵渲染 |
| `evidence/<platform>.json` | 每个平台一份证据登记册（唯一真源） |
| `build-evidence-matrix.py` | 把登记册投影进公共平台矩阵文档的生成节 |
| `record-platform-evidence.py` | 把一次真实运行（安装冒烟）的回执登记进登记册 |

## 腿与状态

每条腿只有一个状态，没有布尔组合：

| 腿 | 含义 |
| --- | --- |
| `build` | 该平台的产物被真正构建出来 |
| `install` | 产物在本平台被安装 |
| `run` | 装好的产物被启动并到达可用状态 |
| `target` | 在真实目标机（真机 / 异构主机）上取证 |

| 状态 | 含义 |
| --- | --- |
| `Proven` | 该腿有真实证据：产物 SHA-256 或 run id |
| `Unsupported` | 该腿在本环境无法取证，必须写明原因与闭合命令 |

状态不是手写断言，而是从证据推导出来的：`Proven` 要求登记册里有一条 `artifact` 或 `run`
证据覆盖该平台**全部**产品；只有 `cross_compile`（交叉编译检查）永远不足以证明任何一腿。
守卫会重新推导状态并与登记册比对，手写的假状态会被拒绝。

## 证据类型

| 类型 | 身份格式 | 能否单独证明一腿 |
| --- | --- | --- |
| `artifact` | `sha256:<64 位十六进制>` | 能 |
| `run` | `run:<run id>` | 能 |
| `cross_compile` | `command:<命令>` | 不能 |

## 与其它守卫的关系

- 平台集合与业务覆盖矩阵一致：五个平台的键取自 `scripts/quality/parity-coverage-guard.py`
  的 `PLATFORM_LEGS`，守卫直接读同一份常量，不复制第二份列表。
- 桌面平台的产品集合必须等于业务覆盖矩阵的表面集合；Android 的产品集合必须覆盖它的表面集合；
  iOS 不交付任何产品。
- 每个产品都必须登记在 `product-identity.properties` 里。
- 平台矩阵文档的表格行与登记册的平台标签必须一致；生成节必须与登记册逐字一致。

## Linux 腿的证据来源

`packaging/linux/build-linux.sh` 产出 deb 与 AppImage，`packaging/linux/smoke-install.sh`
执行安装-启动-卸载冒烟并写出运行回执；`record-platform-evidence.py` 把回执登记进
`scripts/platform/evidence/linux.json`，随后重新生成矩阵节。
