# infiltrator-shared

MusicFrog 各产品复用的基础类型与国际化资源。它提供统一错误码、基础展示工具，以及简体中文和英语文案；Iced、Bevy UI 和移动宿主使用同一套翻译与单次参数插值。用户提供的配置名、节点名和日志正文保持原样。

业务投影由 application 生成，此 crate 不依赖具体业务宿主或 UI 框架。静态文案、动态文案模板和错误说明在这里维护，原生控件负责回放及无障碍标签更新。

验证入口是根目录的 `bash scripts/test.sh`，国际化结构检查使用 `python3 scripts/quality/i18n-guard.py --mode enforce`。
