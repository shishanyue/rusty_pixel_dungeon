//! `TitleScene`：四层星空背景 + Pixel Dungeon 横幅 + 按钮列。
//!
//! 对照 SPD：`scenes/TitleScene.java`（布局层次）、`ui/TitleBackground.java`
//! （四层贴图的帧网格、缩放基准与亮度）、`effects/BannerSprites.java`（横幅源矩形）。
//! M1 背景仅静态摆放；TODO(M2)：视差滚动（SPD `SCROLL_SPEED=15`，各层 1.33 倍递增）
//! 与底部渐暗遮罩。

use bevy::{picking::pointer::PointerButton, prelude::*, window::PrimaryWindow};

use super::text;
use crate::{
    assets::{FontAssets, InterfaceType, InterfacesCollection, SplashType, SplashesCollection},
    states::AppState,
};

pub struct TitleScenePlugin;

impl Plugin for TitleScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::Title),
            (
                spawn_title_root,
                // 资产集合在 Loading 完成后必然存在（bevy_asset_loader 的
                // continue_to_state 保证）；run_if 仅为无资产的 MinimalPlugins
                // 集成测试留出跳过路径，生产环境恒为真
                spawn_title_background.run_if(resource_exists::<SplashesCollection>),
                spawn_title_ui
                    .run_if(resource_exists::<InterfacesCollection>)
                    .run_if(resource_exists::<FontAssets>),
            )
                .chain(),
        )
        .add_systems(
            Update,
            button_hover_feedback.run_if(in_state(AppState::Title)),
        );
    }
}

/// Title 的 UI 根节点标记：UI 生成系统向其挂子节点，集成测试用它断言清理
#[derive(Component)]
pub(crate) struct TitleUiRoot;

// ---------------------------------------------------------------------------
// 布局与素材常量
// ---------------------------------------------------------------------------

/// SPD 一切标题背景尺寸按「窗口高 / 450」定标（`TitleBackground.java:102`）
const DESIGN_HEIGHT: f32 = 450.0;

/// banners.png 的横版标题源矩形：`BannerSprites.java:48`
/// `TITLE_LAND = uvRect(0, 100, 240, 157)`（像素坐标，左上原点；桌面横屏取横版）
const BANNER_TITLE_LAND: Rect = Rect {
    min: Vec2::new(0.0, 100.0),
    max: Vec2::new(240.0, 157.0),
};

/// 横幅显示尺寸：源矩形 240x57 放大 2 倍
const BANNER_DISPLAY_SIZE: Vec2 = Vec2::new(480.0, 114.0);

// 背景层深（后 → 前），层序照抄 TitleBackground 的 add 顺序
const Z_ARCHS: f32 = -40.0;
const Z_BACK_CLUSTERS: f32 = -30.0;
const Z_MID_MIXED: f32 = -20.0;
const Z_FRONT_SMALL: f32 = -10.0;

// 按钮配色：SPD Chrome GREY_BUTTON_TR 的纯色近似 + TITLE_COLOR(0xFFFF44) 描边
const BUTTON_BG: Color = Color::srgba(0.0, 0.0, 0.0, 0.6);
const BUTTON_BG_HOVERED: Color = Color::srgba(0.18, 0.18, 0.18, 0.8);
const BUTTON_BG_PRESSED: Color = Color::srgba(0.32, 0.32, 0.20, 0.9);
const BUTTON_BORDER: Color = Color::srgba(1.0, 1.0, 0.27, 0.5);
const BUTTON_TEXT: Color = Color::WHITE;

/// 帧网格：SPD `TextureFilm(贴图, 帧宽, 帧高)` 的行主序切分
/// （`TitleBackground.java:47-65`；列数 = 贴图宽 / 帧宽 向下取整）
struct FrameGrid {
    cols: u32,
    size: Vec2,
}

impl FrameGrid {
    /// 帧序号 → 源矩形（图像像素坐标，左上原点，同 SPD `uvRect`）
    fn rect(&self, index: u32) -> Rect {
        let col = (index % self.cols) as f32;
        let row = (index / self.cols) as f32;
        Rect {
            min: Vec2::new(col * self.size.x, row * self.size.y),
            max: Vec2::new((col + 1.0) * self.size.x, (row + 1.0) * self.size.y),
        }
    }
}

// 四张贴图的帧网格（实际文件尺寸见注释，均为 2 的幂补白）
/// archs.png 1024x256 → 3 列 x 2 行，6 帧
const ARCH_GRID: FrameGrid = FrameGrid {
    cols: 3,
    size: Vec2::new(333.0, 100.0),
};
/// `back_clusters.png` 512x512 → 1 列 x 2 行，2 帧
const CLUSTER_GRID: FrameGrid = FrameGrid {
    cols: 1,
    size: Vec2::new(450.0, 250.0),
};
/// `mid_mixed.png` 2048x1024 → 7 列 x 4 行，有效 24 帧
const MID_GRID: FrameGrid = FrameGrid {
    cols: 7,
    size: Vec2::new(273.0, 242.0),
};
/// `front_small.png` 1024x512 → 9 列 x 4 行，有效 20 帧
const SMALL_GRID: FrameGrid = FrameGrid {
    cols: 9,
    size: Vec2::new(112.0, 116.0),
};

/// 浮动层静态摆放条目：（帧序号, x 比例, y 比例, 相对缩放, 旋转角度°）。
/// 位置为窗口比例（0,0 = 左上，1,1 = 右下）；SPD 运行期随机散布
/// （`TitleBackground.java` 各 update*Layer），M1 静态版用手调表替代，保持确定性
type FloatingEntry = (u32, f32, f32, f32, f32);

/// 远星团层：SPD 缩放 0.5~1.0、亮度 0.5~0.75
const BACK_CLUSTER_ENTRIES: &[FloatingEntry] = &[
    (0, 0.18, 0.22, 0.90, -12.0),
    (1, 0.72, 0.30, 1.00, 8.0),
    (0, 0.42, 0.68, 0.95, 18.0),
    (1, 0.88, 0.80, 0.85, -6.0),
];

/// 中景杂物层：SPD 缩放 0.75~1.75、亮度 0.9
const MID_MIXED_ENTRIES: &[FloatingEntry] = &[
    (0, 0.12, 0.14, 1.00, 10.0),
    (3, 0.55, 0.08, 0.85, -14.0),
    (7, 0.85, 0.28, 1.10, 6.0),
    (11, 0.30, 0.42, 0.90, -8.0),
    (14, 0.68, 0.60, 1.20, 16.0),
    (18, 0.14, 0.76, 1.00, -18.0),
    (21, 0.50, 0.88, 1.15, 5.0),
    (23, 0.90, 0.90, 0.90, -10.0),
];

/// 前景小物层：SPD 缩放 2.0~2.5、亮度 1.0
const FRONT_SMALL_ENTRIES: &[FloatingEntry] = &[
    (2, 0.25, 0.18, 2.10, -10.0),
    (6, 0.62, 0.38, 2.40, 12.0),
    (10, 0.08, 0.58, 2.00, 7.0),
    (15, 0.80, 0.72, 2.30, -15.0),
    (19, 0.38, 0.92, 2.20, 9.0),
];

// ---------------------------------------------------------------------------
// 场景生成
// ---------------------------------------------------------------------------

fn spawn_title_root(mut commands: Commands) {
    commands.spawn((
        TitleUiRoot,
        DespawnOnExit(AppState::Title),
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        },
    ));
}

/// 四层背景静态摆放（世界坐标精灵，UI 永远压在其上）
fn spawn_title_background(
    mut commands: Commands,
    splashes: Res<SplashesCollection>,
    window: Single<&Window, With<PrimaryWindow>>,
) {
    let size = Vec2::new(window.width(), window.height());
    let scale = size.y / DESIGN_HEIGHT;

    spawn_arch_layer(
        &mut commands,
        splashes.get(SplashType::TitleArchs),
        size,
        scale,
    );
    spawn_floating_layer(
        &mut commands,
        splashes.get(SplashType::TitleBackClusters),
        &CLUSTER_GRID,
        BACK_CLUSTER_ENTRIES,
        size,
        scale,
        0.55,
        Z_BACK_CLUSTERS,
    );
    spawn_floating_layer(
        &mut commands,
        splashes.get(SplashType::TitleMidMixed),
        &MID_GRID,
        MID_MIXED_ENTRIES,
        size,
        scale,
        0.85,
        Z_MID_MIXED,
    );
    spawn_floating_layer(
        &mut commands,
        splashes.get(SplashType::TitleFrontSmall),
        &SMALL_GRID,
        FRONT_SMALL_ENTRIES,
        size,
        scale,
        1.0,
        Z_FRONT_SMALL,
    );
}

/// 拱门层平铺整屏：行距 95、横向搭接 9（`TitleBackground.java:317-336` 的静态化）；
/// 压暗到 0.7 近似 SPD 叠加的渐暗遮罩效果
fn spawn_arch_layer(commands: &mut Commands, image: Handle<Image>, size: Vec2, scale: f32) {
    // SPD 抽帧权重 {5,5,2,2,2,2} 偏向帧 0/1，静态版用固定循环序列近似
    const ARCH_PATTERN: [u32; 8] = [0, 1, 2, 0, 1, 3, 4, 5];

    let col_step = (ARCH_GRID.size.x - 9.0) * scale;
    let row_step = 95.0 * scale;
    let cols = (size.x / col_step).ceil() as i32 + 1;
    let rows = (size.y / row_step).ceil() as i32 + 1;

    for row in 0..rows {
        for col in 0..cols {
            let frame = ARCH_PATTERN[((row * cols + col) as usize) % ARCH_PATTERN.len()];
            // 精灵锚点默认居中，把左上角平铺坐标换算为中心点（世界坐标 +y 朝上）
            let center = Vec2::new(
                -size.x / 2.0 + col as f32 * col_step + ARCH_GRID.size.x * scale / 2.0,
                size.y / 2.0 - row as f32 * row_step - ARCH_GRID.size.y * scale / 2.0,
            );
            commands.spawn((
                DespawnOnExit(AppState::Title),
                Sprite {
                    image: image.clone(),
                    rect: Some(ARCH_GRID.rect(frame)),
                    color: Color::srgb(0.7, 0.7, 0.7),
                    ..default()
                },
                Transform::from_xyz(center.x, center.y, Z_ARCHS).with_scale(Vec3::splat(scale)),
            ));
        }
    }
}

/// 按静态摆放表生成一层浮动贴片；亮度经精灵 color 乘算（同 SPD brightness）
fn spawn_floating_layer(
    commands: &mut Commands,
    image: Handle<Image>,
    grid: &FrameGrid,
    entries: &[FloatingEntry],
    size: Vec2,
    base_scale: f32,
    brightness: f32,
    z: f32,
) {
    for &(frame, x_frac, y_frac, scale_mul, angle_deg) in entries {
        commands.spawn((
            DespawnOnExit(AppState::Title),
            Sprite {
                image: image.clone(),
                rect: Some(grid.rect(frame)),
                color: Color::srgb(brightness, brightness, brightness),
                ..default()
            },
            Transform {
                translation: Vec3::new((x_frac - 0.5) * size.x, (0.5 - y_frac) * size.y, z),
                rotation: Quat::from_rotation_z(angle_deg.to_radians()),
                scale: Vec3::splat(base_scale * scale_mul),
            },
        ));
    }
}

/// 横幅 + 按钮列：横幅居中于上部 45% 区域
/// （`TitleScene.java:110` `topRegion = h*0.45`），按钮列紧随其下
fn spawn_title_ui(
    mut commands: Commands,
    interfaces: Res<InterfacesCollection>,
    fonts: Res<FontAssets>,
    root: Single<Entity, With<TitleUiRoot>>,
) {
    let banner = interfaces.get(InterfaceType::Banners);
    let font = fonts.pixel.clone();

    commands.entity(*root).with_children(|parent| {
        parent
            .spawn(Node {
                width: percent(100),
                height: percent(45),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|banner_area| {
                banner_area.spawn((
                    ImageNode {
                        image: banner,
                        rect: Some(BANNER_TITLE_LAND),
                        ..default()
                    },
                    Node {
                        width: px(BANNER_DISPLAY_SIZE.x),
                        height: px(BANNER_DISPLAY_SIZE.y),
                        ..default()
                    },
                ));
            });

        parent
            .spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(16),
                margin: UiRect::top(px(24)),
                ..default()
            })
            .with_children(|buttons| {
                buttons
                    .spawn(styled_button(text::BTN_ENTER_DUNGEON, font.clone()))
                    .observe(on_enter_dungeon_clicked);
                buttons
                    .spawn(styled_button(text::BTN_QUIT, font))
                    .observe(on_quit_clicked);
            });
    });
}

/// SPD 风格按钮：暗色半透明底 + 标题黄描边 + 像素字体
fn styled_button(label: &'static str, font: Handle<Font>) -> impl Bundle {
    (
        Button,
        Node {
            width: px(320),
            height: px(48),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(2)),
            ..default()
        },
        BackgroundColor(BUTTON_BG),
        BorderColor::all(BUTTON_BORDER),
        children![(
            Text::new(label),
            TextFont {
                font: font.into(),
                font_size: FontSize::Px(22.0),
                ..default()
            },
            TextColor(BUTTON_TEXT),
        )],
    )
}

// ---------------------------------------------------------------------------
// 交互
// ---------------------------------------------------------------------------

fn on_enter_dungeon_clicked(
    click: On<Pointer<Click>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if click.button == PointerButton::Primary {
        next_state.set(AppState::InGame);
    }
}

fn on_quit_clicked(click: On<Pointer<Click>>, mut app_exit: MessageWriter<AppExit>) {
    if click.button == PointerButton::Primary {
        app_exit.write(AppExit::Success);
    }
}

/// 悬停/按下的底色反馈（观察者只管点击，颜色反馈走 `Interaction` 轮询）
fn button_hover_feedback(
    mut buttons: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, mut background) in &mut buttons {
        background.0 = match interaction {
            Interaction::Pressed => BUTTON_BG_PRESSED,
            Interaction::Hovered => BUTTON_BG_HOVERED,
            Interaction::None => BUTTON_BG,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 帧网格行主序切分与 SPD `TextureFilm` 一致
    #[test]
    fn frame_grid_is_row_major() {
        assert_eq!(ARCH_GRID.rect(0), Rect::new(0.0, 0.0, 333.0, 100.0));
        // 帧 4 = 第 2 行第 2 列
        assert_eq!(ARCH_GRID.rect(4), Rect::new(333.0, 100.0, 666.0, 200.0));
        assert_eq!(CLUSTER_GRID.rect(1), Rect::new(0.0, 250.0, 450.0, 500.0));
    }

    /// 横幅源矩形照抄 `BannerSprites.java:48` `TITLE_LAND = uvRect(0,100,240,157)`
    #[test]
    fn banner_rect_matches_spd() {
        assert_eq!(BANNER_TITLE_LAND, Rect::new(0.0, 100.0, 240.0, 157.0));
    }

    /// 摆放表引用的帧序号不越过各贴图的有效帧数
    #[test]
    fn floating_entries_within_frame_counts() {
        assert!(BACK_CLUSTER_ENTRIES.iter().all(|e| e.0 < 2));
        assert!(MID_MIXED_ENTRIES.iter().all(|e| e.0 < 24));
        assert!(FRONT_SMALL_ENTRIES.iter().all(|e| e.0 < 20));
    }
}
