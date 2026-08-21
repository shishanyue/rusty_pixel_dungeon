# 12 · 本地化与消息域计划

**文件所有权**：`src/assets/`（含 `assets.rs`）与 `src/utils.rs` 中 PropertyPath 族。
禁止改 `main.rs`/`Cargo.toml`/其他域目录。

**参考实现**：`shattered-pixel-dungeon/core/.../messages/Messages.java`；
资产：`assets/messages/{actors,items,journal,levels,misc,plants,scenes,ui,windows}/
<cat>[_lang].properties`（27 语言）、`assets/languages/languages.json`。

## SPD 语义

- 9 个 bundle 目录，key 全小写（`Messages.get` 前先 `toLowerCase`），
  按 bundle 顺序查找，miss 返回 `!!!NO TEXT FOUND!!!` 风格哨兵。
- 语言回退：请求语言 → 英语基线（`actors.properties` 无后缀即英语）。
- 占位格式：libgdx `I18NBundle.format` 的 `{0}` 风格 + SPD 自有的性别/复数扩展
  （M1 只做 `{N}` 参数替换，性别/选择语法记 TODO 到 backlog）。

## 设计

1. 地基已修复消息文件动态注册（英文基线）。本域扩展为**按选定语言注册两份**
   （`<cat>.properties` + `<cat>_<code>.properties`，code 来自 languages.json 的
   `Language.code`，zh → `_zh`）。
2. `Resource Messages { chain: Vec<Arc<HashMap<String,String>>> }`：
   查找顺序 = [语言文件…, 英文基线…]；`get(key)` / `format(key, &[args])`。
   构建时机：LoadingState 完成后（`OnEnter(AppState::Title)` 或
   bevy_asset_loader 的 `init_resource_after_loading_state`）。
3. `LanguageServer`（languages.json → 语言元数据）修复审计 C2：注册构建系统，
   重命名 `match_code` → `get(lang) -> &Language`，去掉 `.iter().into_iter()`。
4. `Settings.local_code` 默认 `"en"`；切语言 = 更新 Settings + 重进 Loading 状态
   重载消息集合（M1 允许仅实现"启动时选定"，运行时切换记 TODO）。
5. 纯函数层（属性合并、key 归一化、`{N}` 替换）与 Bevy 层分离，单测打纯函数层，
   直接用 `java-properties` 解析 `assets/messages/` 真实文件对拍。

## 验收

- 单测：`get("actors.rat.name")` 类真实 key 在 en 与 zh 下取值正确；
  miss 哨兵；`{0}/{1}` 替换；key 大小写归一。
- `cargo clippy` 无新告警；不引入新依赖。

## 进度

- [x] 纯函数层 + 单测
- [x] Messages 资源 + 构建时机
- [x] LanguageServer 修复
- [x] 语言后缀注册

## 实现笔记（M1 交付）

**模块布局**：`src/assets/messages.rs` 新增（纯函数层 + `Messages` 资源 + `MessagesPlugin`）。
消息集合的装载与 `PropertiesAsset` 初始化从 `AssetsPlugin` 移入 `MessagesPlugin`
（`configure_loading_state`，与 `LanguagePlugin` 一样必须在 `add_loading_state` 之后注册）。

**miss 哨兵**：与 SPD `Messages.NO_TEXT_FOUND` 逐字一致 `!!!NO TEXT FOUND!!!`；
`format` miss 时返回哨兵且不做参数替换（Java 同义）。key 查找前 `to_lowercase`。

**回退链**：`Messages.chain = [语言变体 bundle…, 英文基线 bundle…]`，两段内部均按
`MessageType` 枚举序（= SPD `prop_files` 顺序）。SPD 是每个 bundle 内部做 locale
回退，本实现是"变体段 → 基线段"全局链——key 均带分类前缀（`actors.` 等），
跨分类无重名，两种顺序结果一致。

**缺失文件处理**：`register_message_dynamic_assets`（`PreStartup`）按
`Settings.local_code` 追加 `<cat>_<code>.properties`，注册前用
`FileAssetReader::get_base_path()/assets` 做磁盘存在性检查（与 asset server
的路径解析一致），缺失分类 `warn!` 后跳过（注册不存在路径会卡死 loading state），
该分类自然回退英文基线。`en` 无变体后缀，即基线本身。构建 `Messages` 时用
`MessagesCollection::contains` 重新判定变体是否装载，不再碰磁盘。

**占位符（与计划的偏差，已扩展）**：计划按 libgdx `I18NBundle` 记 `{N}` 风格，但
现版 SPD 的 `Messages.format` 实为 `String.format`，仓库内 properties 文件全部使用
printf 风格（`%s`/`%d` 五千余处、`%N$s` 位置参数、`%%`、`%.2f`），`{0}` 零处。
`format_args` 因此同时支持 `{N}` 与 printf 子集（`%s`/`%d`/`%f`、`%N$s`、`%%`、
精度忽略）；索引越界/无法识别的占位符原样保留。性别/复数扩展语法仍记 backlog。

**编码修复（顺带）**：`java-properties` 固定按 windows-1252 解码，直读 UTF-8 中文
必乱码；`parse_properties` 先把非 ASCII 字符预转义为 `\uXXXX` 再解析。该 crate 的
`\uXXXX` 解码拒绝代理对，增补平面字符明确报错（消息文件已核实不含）。

**集成测试**：`messages_resource_builds_after_loading`（`MinimalPlugins` +
`StatesPlugin` + `AssetPlugin`，无窗口）验证 zh 下装载后 `Messages` 资源构建、
中文取值与 `LanguageServer::get_by_code` 映射。运行时切语言（重进 Loading 重载）
仍记 TODO（M1 启动时选定）。
