//! 音乐音频域：状态驱动的背景音乐（见 `docs/plans/18-audio-music.md`）。
//!
//! 对照 SPD：`SPD-classes/.../noosa/audio/Music.java` 的 play/stop 语义——
//! `play(assetName, looping)`（54-79 行）先 `stop()` 掉旧曲再起新曲，任何时刻
//! 至多一首在播。本域用「标记组件 + 生成前清扫」复刻该不变量：
//! 每次状态进入先 despawn 全部 [`GameMusic`] 实体，再生成唯一新实体。
//!
//! 实体管理取舍：bevy 0.19 只对**没有** `AudioSink` 的实体开始播放
//! （`bevy_audio/src/audio_output.rs` `play_queued_audio_system` 的
//! `Without<AudioSink>` 过滤），常驻实体仅替换 `AudioPlayer` 句柄不会重播，
//! 还得手动摘 `AudioSink`——脆弱且无官方背书；官方 `examples/audio/soundtrack.rs`
//! 亦采用「旧实体销毁 + 新实体生成」模式，故选后者。
//!
//! M1 范围：每状态一首循环曲（Title → `THEME_1`，`InGame` → `SEWERS_1`）。
//! SPD 的多轨 interlude（`playTracks` 按概率排队轮播）记 TODO，见计划文档笔记。

use bevy::{audio::Volume, prelude::*};

use crate::{
    assets::{MusicCollection, MusicType},
    states::AppState,
};

/// 背景音乐音量（线性标度，1.0 = 原始响度）。
/// TODO(M7)：接 `Settings` 的音乐音量/开关字段（对照 SPD `SPDSettings.musicVol`
/// 与 `Music.INSTANCE.volume()`；`setting.rs` 本里程碑禁改，先用固定常量）。
const MUSIC_VOLUME: f32 = 0.5;

/// 当前背景音乐实体标记：任何时刻至多存在一个（[`stop_music`] 清扫保证）。
#[derive(Component)]
pub(crate) struct GameMusic;

pub struct GameMusicPlugin;

impl Plugin for GameMusicPlugin {
    fn build(&self, app: &mut App) {
        // 每个有音乐的状态挂同一对系统，chain 保证「先清扫旧曲、再起新曲」。
        // 清扫与起曲拆成两个系统：清扫不依赖资产集合，无条件运行，
        // 使防叠播不变量在无资产的 MinimalPlugins 测试环境下也成立且可直接单测。
        for state in [AppState::Title, AppState::InGame] {
            app.add_systems(
                OnEnter(state),
                (
                    stop_music,
                    // 集合在 Loading 完成后必然存在（bevy_asset_loader 的
                    // continue_to_state 保证），生产环境恒为真；run_if 仅为
                    // 无资产的 MinimalPlugins 集成测试留跳过路径（同 scenes 域模式）
                    play_state_music.run_if(resource_exists::<MusicCollection>),
                )
                    .chain(),
            );
        }
    }
}

/// 选曲纯函数：状态 → 曲目。
///
/// SPD 对照：Title 用 `THEME_1`（`TitleScene.java:89-92`，SPD 实为
/// `THEME_1`/`THEME_2` 双轨轮播）；下水道用 `SEWERS_1`（`SewerLevel.java:70-83`，
/// SPD 实为 `SEWERS_1/2/3` 概率轮播）。M1 各取首曲循环，多轨记 TODO。
pub fn music_for_state(state: AppState) -> Option<MusicType> {
    match state {
        AppState::Loading => None,
        AppState::Title => Some(MusicType::Theme1),
        AppState::InGame => Some(MusicType::Sewers1),
    }
}

/// 清扫全部旧音乐实体（防叠播；对应 SPD `Music.stop()` 的 dispose 语义，
/// `Music.java:257-262`）。despawn 连带释放 `AudioSink`，播放即刻停止。
fn stop_music(mut commands: Commands, playing: Query<Entity, With<GameMusic>>) {
    for entity in &playing {
        commands.entity(entity).despawn();
    }
}

/// 按进入的状态起新曲：循环播放 + 固定音量。
/// `OnEnter` 调度运行时 `State<AppState>` 已是新状态，可直接读取选曲。
fn play_state_music(
    mut commands: Commands,
    music: Res<MusicCollection>,
    state: Res<State<AppState>>,
) {
    let Some(track) = music_for_state(*state.get()) else {
        return;
    };
    commands.spawn((
        GameMusic,
        AudioPlayer::new(music.get(track)),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(MUSIC_VOLUME)),
    ));
}

#[cfg(test)]
mod tests {
    use bevy::{ecs::system::RunSystemOnce, state::app::StatesPlugin};

    use super::*;

    /// 选曲表与 SPD 调用形态对照：`TitleScene.java:89-92`（`THEME_1`）、
    /// `SewerLevel.java:70-83`（`SEWERS_1`）；Loading 无音乐
    #[test]
    fn music_for_state_matches_spd_tracks() {
        assert_eq!(music_for_state(AppState::Loading), None);
        assert_eq!(music_for_state(AppState::Title), Some(MusicType::Theme1));
        assert_eq!(music_for_state(AppState::InGame), Some(MusicType::Sewers1));
    }

    fn count_music(world: &mut World) -> usize {
        world
            .query_filtered::<(), With<GameMusic>>()
            .iter(world)
            .count()
    }

    /// 测试策略说明：`MusicCollection` 的 `AssetCollection` 字段私有，测试无法
    /// 构造假集合注入，故防叠播断言拆两层——本测试对清扫系统做 World 级单测
    /// （手动 spawn 两个带标记实体跑清扫，断言归零），下一个测试验证插件级
    /// 调度接线。清扫后紧接的起曲系统只 spawn 一个实体，两者合并即
    /// 「任何时刻至多一个音乐实体」的不变量。
    #[test]
    fn stop_music_sweeps_all_marked_entities() {
        let mut world = World::new();
        world.spawn(GameMusic);
        world.spawn(GameMusic);
        assert_eq!(count_music(&mut world), 2);

        world.run_system_once(stop_music).unwrap();
        assert_eq!(count_music(&mut world), 0, "清扫后不得残留音乐实体");
    }

    /// 插件级集成：MinimalPlugins 无 `MusicCollection` → 起曲被 `run_if` 跳过、
    /// 清扫照常运行。每次切状态前手工补位一个"在播"实体模拟旧曲，
    /// 断言反复切换后实体不累积（防叠播）。
    #[test]
    fn state_cycling_never_accumulates_music_entities() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<AppState>()
            .add_plugins(GameMusicPlugin);
        // 首帧：进入 Loading（无音乐系统挂载）
        app.update();

        for state in [
            AppState::Title,
            AppState::InGame,
            AppState::Title,
            AppState::InGame,
        ] {
            // 模拟上一状态遗留的在播音乐（无资产环境起曲被跳过，手工 spawn 补位）
            app.world_mut().spawn(GameMusic);
            app.world_mut()
                .resource_mut::<NextState<AppState>>()
                .set(state);
            app.update();

            let remaining = count_music(app.world_mut());
            assert!(
                remaining <= 1,
                "进入 {state:?} 后音乐实体叠播：{remaining} 个"
            );
            // 本环境起曲被 run_if 跳过，清扫后应精确归零
            assert_eq!(remaining, 0, "无资产环境下清扫后应无音乐实体");
        }
    }
}
