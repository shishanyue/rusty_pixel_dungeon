# 18 · 音乐音频域计划（M7 提前的独立项）

**文件所有权**：新建 `src/audio.rs` + `src/audio/` 目录（`main.rs` 注册由协调者
合流时接入，你不碰）。禁止改 `Cargo.toml`（bevy 内置音频已可用：vorbis 默认 +
mp3 特性已开）、其他域目录。

**参考实现**：
- SPD：`SPD-classes/.../noosa/audio/Music.java`（播放/循环/音量/切换语义）与
  `core/.../scenes/TitleScene.java`、`GameScene.java` 中 `Music.INSTANCE.play*`
  的调用形态（title 用 THEME_1，下水道用 SEWERS_1 等）
- 本项目资产：`MusicCollection`（`src/assets.rs`，`get(MusicType::Theme1)` 取
  `Handle<AudioSource>`）；bevy 0.19 音频：`AudioPlayer`/`PlaybackSettings`
  （查 `~/GitHub/bevy/examples/audio/`）

## 目标

1. `GameMusicPlugin`：状态驱动的音乐切换——`OnEnter(Title)` 播 `Theme1` 循环、
   `OnEnter(InGame)` 播 `Sewers1` 循环、状态退出时停掉旧曲（一个常驻"当前音乐"
   实体或资源管理句柄，避免叠播）。
2. 音量：先读 `Settings` 的新字段？——不加字段（`setting.rs` 禁改），M7 再接；
   先用固定音量常量（0.5 左右）+ 顶部 const 注明 M7 接 Settings。
3. SPD 的多轨 interlude/分层播放记 TODO（笔记里列出 Music.java 对应行号），
   M1 范围只做"每状态一首循环曲"。

## 强制验收

- `cargo check/clippy --all-targets` 零错误零新告警；`cargo test` 全绿。
- 集成测试（MinimalPlugins，无音频输出设备也能跑组件断言）：进入 Title 生成唯一
  音乐实体、切到 InGame 旧实体清理且新实体持有 `Sewers1` 句柄（比较 Handle id；
  无资产环境用 run_if 跳过的话，测试注入假 `MusicCollection` 或改为直接单测
  选曲函数——取可行者，笔记说明）。
- 音乐实体不得在状态反复切换后累积（防叠播断言）。

## 进度

- [x] GameMusicPlugin 状态驱动切换
- [x] 防叠播 + 测试
- [x] 笔记（多轨 TODO 行号）

## 实现笔记（M1 交付）

### 实体管理取舍：标记组件 + 生成前清扫

两个候选方案里选了「`GameMusic` 标记组件 + 每次 `OnEnter` 先清扫再 spawn」，
放弃「常驻实体换 `AudioPlayer` 句柄」。依据：bevy 0.19 只对**没有** `AudioSink`
的实体开始播放（`bevy_audio/src/audio_output.rs` 的 `play_queued_audio_system`
带 `Without<AudioSink>` 过滤），常驻实体仅替换句柄不会触发重播，还得手动摘
`AudioSink` 组件，脆弱且无官方背书；官方 `examples/audio/soundtrack.rs` 换曲
也是「旧实体销毁 + 新实体生成」。despawn 连带 drop `AudioSink`，播放即停，
正对应 SPD `Music.stop()` 的 `player.dispose()`（`Music.java:257-262`）。

清扫（`stop_music`）与起曲（`play_state_music`）拆成两个系统 `chain()` 挂在
`OnEnter(Title)` / `OnEnter(InGame)`：清扫不依赖资产集合、无条件运行；起曲挂
`run_if(resource_exists::<MusicCollection>)`（与 scenes 域同一模式：生产环境
Loading 完成后恒真，MinimalPlugins 测试环境跳过）。选曲逻辑抽成纯函数
`music_for_state(AppState) -> Option<MusicType>`（Loading → None、
Title → `Theme1`、InGame → `Sewers1`）。

### 测试策略

`MusicCollection` 的 `AssetCollection` 字段私有，测试无法构造假集合注入，
计划里的「比较新实体 Sewers1 句柄」不可行，按备选方案落地为三层：

1. `music_for_state` 纯函数单测（选曲表对照 SPD 调用点）；
2. 清扫系统 World 级单测：手动 spawn 两个带 `GameMusic` 的实体，
   `run_system_once(stop_music)` 后断言归零；
3. 插件级集成（MinimalPlugins + StatesPlugin）：每次切状态前手工 spawn 一个
   `GameMusic` 模拟"上一状态遗留在播曲"，Title ↔ InGame 反复切换后断言实体
   数恒为 0（清扫照跑、起曲被 run_if 跳过）——覆盖"反复切换不累积"。

清扫后紧接的起曲只 spawn 一个实体，2+3 合并即"任何时刻至多一个音乐实体"。

### 音量

固定常量 `MUSIC_VOLUME = 0.5`（`PlaybackSettings::LOOP.with_volume(Volume::Linear(..))`）。
TODO(M7)：接 `Settings` 音乐音量/开关（对照 SPD `SPDSettings.musicVol` 与
`Music.INSTANCE.volume()`；本里程碑 `setting.rs` 禁改）。

### SPD 多轨 interlude 机制 TODO 行号清单（M1 不实现）

`SPD-classes/.../noosa/audio/Music.java`：

- `playTracks(tracks, chances, shuffle)` 多轨入口（含"相同/平移轨表不重开"
  判定）：81-139 行
- `trackLooper`（OnCompletionListener，单曲播完接下一首）：169-188 行
- `playNextTrack`（按概率重填队列 + 可选 shuffle）：190-211 行
- `fadeOut(duration, onComplete)`：141-150 行；淡出推进 `update()`：152-167 行；
  `volumeWithFade()`：271-277 行
- `pause()/resume()`（后台暂停恢复）：242-255 行

调用形态：

- `TitleScene.java:89-92`——`playTracks({THEME_1, THEME_2}, {1,1}, false)`，
  M1 简化为 `Theme1` 单曲循环
- `SewerLevel.java:70-73`——`SEWER_TRACK_LIST`（SEWERS_1/2/2/1/3/3）+
  `SEWER_TRACK_CHANCES`（1/1/0.5/0.25/1/0.5）；`playLevelMusic()` 75-85 行
  （含 Ghost 任务/拿到护符时的 `SEWERS_TENSE`/`THEME_FINALE` 分支），
  M1 简化为 `Sewers1` 单曲循环
