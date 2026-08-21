//! 动画剪辑：SPD `noosa/MovieClip.java` 内嵌类 `Animation` 的数据层等价物，
//! 以及 `sprites/CharSprite.java` 的四剪辑约定。
//!
//! Java 版把 fps 立即折算成 `delay = 1f / fps` 存储（L120-L123）；Rust 数据表
//! 保留原始 fps 以便与 Java 源逐值对拍，播放间隔由 [`AnimClip::frame_delay`]
//! 现算——数值等价，播放循环（`updateAnimation` L57-L91）属下波接线。

/// 单条动画剪辑（`MovieClip.Animation`，Java L114-L141）。
///
/// - `fps`：构造参数 `Animation(int fps, boolean looped)`（L120）；
/// - `frames`：帧网格内的帧索引序列（`frames(TextureFilm, Object...)`
///   L130-L136，允许重复索引拼节奏）；
/// - `looped`：播完回到首帧循环，否则停在末帧（`updateAnimation` L65-L71）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimClip {
    /// 帧率（帧/秒）。
    pub fps: u32,
    /// 帧索引序列（消费方经 `FrameGrid::frame_rect` 换算像素矩形）。
    pub frames: &'static [u32],
    /// 是否循环播放。
    pub looped: bool,
}

impl AnimClip {
    /// 构造剪辑；`fps == 0` 或空帧序列直接编译失败（const 上下文断言），
    /// Java 侧 `delay > 0` 才推进动画（`updateAnimation` L58）。
    #[must_use]
    pub const fn new(fps: u32, frames: &'static [u32], looped: bool) -> Self {
        assert!(fps > 0, "fps 必须为正");
        assert!(!frames.is_empty(), "剪辑至少一帧");
        Self {
            fps,
            frames,
            looped,
        }
    }

    /// 单帧停留秒数（Java L122 `delay = 1f / fps`）。
    #[must_use]
    pub fn frame_delay(&self) -> f32 {
        1.0 / self.fps as f32
    }
}

/// 角色四剪辑集（`CharSprite.java` L90-L95 字段约定中本波移植的四条：
/// idle/run/attack/die；operate/zap 等扩展剪辑随后续需要再补）。
///
/// 语义（`CharSprite` 播放入口）：`idle()` L213-L215、`move()` 播 run
/// L217-L232、`attack()` L254-L262、`die()` L309-L319（die 不可被打断，
/// `play` 覆写 L136-L142）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CharAnimSet {
    /// 待机（循环）。
    pub idle: AnimClip,
    /// 移动（循环）。
    pub run: AnimClip,
    /// 攻击（单次，播完回 idle：`onComplete` L849-L852）。
    pub attack: AnimClip,
    /// 死亡（单次，停在末帧）。
    pub die: AnimClip,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `frame_delay` 与 Java `delay = 1f / fps`（L122）等价。
    #[test]
    fn frame_delay_matches_java_formula() {
        assert_eq!(AnimClip::new(1, &[0], true).frame_delay(), 1.0);
        assert_eq!(AnimClip::new(20, &[0], false).frame_delay(), 1.0 / 20.0);
        assert_eq!(AnimClip::new(15, &[0], false).frame_delay(), 1.0 / 15.0);
    }

    /// 剪辑保留原始帧序列（含重复索引）与循环标记。
    #[test]
    fn clip_preserves_frames_and_loop_flag() {
        let clip = AnimClip::new(2, &[0, 0, 0, 1], true);
        assert_eq!(clip.frames, &[0, 0, 0, 1]);
        assert_eq!(clip.frames.len(), 4);
        assert!(clip.looped, "idle 类剪辑循环");
        assert!(!AnimClip::new(10, &[3], false).looped, "die 类剪辑单次");
    }
}
