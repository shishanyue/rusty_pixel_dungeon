//! 战斗公式纯核：命中/伤害/护甲掷值，逐行对照 SPD `Char.java`
//! （下文行号未注明文件者均指该文件）。
//!
//! ## 随机语义等价方案
//!
//! 随机源一律显式传 `&mut impl Rng`（`docs/plans/01` 确定性纪律），生成器与
//! Java `java.util.Random` 不同，**不追求位流对齐**；对齐的是公式形态：
//!
//! - `Random.Float()`（`Random.java` L77-79）均匀半开 `[0, 1)`——rand 0.10 的
//!   f32 `StandardUniform` 与 Java `nextFloat()` 同构（均为 24 位分辨率的
//!   `k / 2^24`，可取 0 永不取 1）；
//! - 掷值**次数与顺序**逐一对应（命中先掷攻方后掷守方；`NormalIntRange`
//!   恒消耗两次 `Float()`）；
//! - 乘数施加位置、`(int)` 向零截断、f32 运算序照抄 Java（Java `float`
//!   算术同为 IEEE-754，二者逐位一致）。
//!
//! 本模块与 `levels::random` 的工具有意重复：领域文件所有权是硬边界且两域
//! 并行开发，公式与其对拍测试必须落在本域内；候选的 utils 级统一留给协调者。

use rand::{Rng, RngExt};

use super::char_stats::CharStats;

/// 无限命中阈值（L617 `INFINITE_ACCURACY = 1_000_000`）：命中技能达到即必中
/// （除非对面同时无限闪避）。
pub const INFINITE_ACCURACY: f32 = 1_000_000.0;
/// 无限闪避阈值（L618 `INFINITE_EVASION = 1_000_000`）：闪避技能达到即必闪，
/// 判定**优先于**无限命中（L641-643 注释 "infinite evasion beats infinite
/// accuracy"）。
pub const INFINITE_EVASION: f32 = 1_000_000.0;

/// `Random.Float()`（`Random.java` L77-79）：均匀 `[0, 1)`。
fn float01(rng: &mut impl Rng) -> f32 {
    rng.random::<f32>()
}

/// `Random.Float(max)`（`Random.java` L87-89）：均匀 `[0, max)`，实现为
/// `Float() * max`。`max = 0` 时**仍消耗一次掷值**并必得 0——命中公式的
/// acc=0 / evasion=0 边界依赖该语义。
pub fn float_below(rng: &mut impl Rng, max: f32) -> f32 {
    float01(rng) * max
}

/// `Random.NormalIntRange(min, max)`（`Random.java` L138-140）：闭区间
/// `[min, max]` 三角分布（两次均匀掷值取和，越靠区间中点越常见）。
///
/// - **恒**消耗两次 `Float()`，即使 `min == max`（区别于 `Random.Int(max)`
///   在 `max <= 0` 时的零消耗，`Random.java` L120-124）；
/// - 运算序照抄 Java：`(f1 + f2) * span / 2`（先乘区间宽再除 2），
///   `(int)` 向零截断（Rust `as i32` 同义）；
/// - 上界可达：两掷同取最大值 `1 - 2⁻²⁴` 时 `(2 - 2⁻²³) * span / 2 < span`，
///   截断后恰为 `span - 1`，加 `min` 即 `max`。
pub fn normal_int_range(rng: &mut impl Rng, min: i32, max: i32) -> i32 {
    let sum = float01(rng) + float01(rng);
    min + ((sum * (max - min + 1) as f32) / 2.0) as i32
}

/// 命中判定（`Char.hit` L624-690）剥除 Buff/天赋/饰品乘子后的纯核：
///
/// ```text
/// if defStat >= INFINITE_EVASION  → miss      // L643-645（不掷随机数）
/// if acuStat >= INFINITE_ACCURACY → hit       // L646-649（不掷随机数）
/// acuRoll = Float(acuStat) * accMulti         // L651 掷值 + L665 乘数
/// defRoll = Float(defStat)                    // L667（后掷，掷序影响随机流）
/// return acuRoll >= defRoll                   // L683，平手判中
/// ```
///
/// 要点：
/// - `acc_multi` 乘在**掷出的值**上（先掷后乘，L665），不是乘在掷值上限上；
///   分布相同但浮点舍入不同，且它**不参与** L646 的无限命中阈值判定。
///   魔法攻击的便捷重载传 2.0（L620-622），偷袭必中走 `INFINITE_ACCURACY`
///   （L633-635，属 M4）；
/// - Bless/Hex/Daze/冠军怪/登顶挑战/雪貂草乘子（L652-681）皆 Buff/饰品域，M4 接入；
/// - `attack_skill`/`defense_skill` 取自组件字段，对应 Java 动态方法
///   `attacker.attackSkill(defender)` / `defender.defenseSkill(attacker)`
///   （L625-626）的已折算值。
#[must_use]
pub fn hit(attacker: &CharStats, defender: &CharStats, acc_multi: f32, rng: &mut impl Rng) -> bool {
    hit_with_skills(
        attacker.attack_skill as f32,
        defender.defense_skill as f32,
        acc_multi,
        rng,
    )
}

/// [`hit`] 的标量形态：直接给已折算的命中/闪避技能值，供边界测试与 M4 装备域
/// （折算含武器精度、`max(1, …)` 下限等，`Hero.java` L555-557/L604）复用。
#[must_use]
pub fn hit_with_skills(
    attack_skill: f32,
    defense_skill: f32,
    acc_multi: f32,
    rng: &mut impl Rng,
) -> bool {
    if defense_skill >= INFINITE_EVASION {
        return false; // L643-645：无限闪避优先，且不消耗随机数
    }
    if attack_skill >= INFINITE_ACCURACY {
        return true; // L646-649：无限命中，同样不消耗随机数
    }
    let acu_roll = float_below(rng, attack_skill) * acc_multi; // L651 + L665
    let def_roll = float_below(rng, defense_skill); // L667
    acu_roll >= def_roll // L683
}

/// 伤害掷值：伤害域上的 `NormalIntRange`。
///
/// 覆盖对象的 Java 原文：Rat `damageRoll()`（`Rat.java` L54-57）、Snake
/// （`Snake.java` L48-51）、Crab（`Crab.java` L45-48）；徒手英雄经
/// `Hero.damageRoll`（`Hero.java` L663-696）→ `RingOfForce.damageRoll`
/// （`RingOfForce.java` L105）→ `heroDamageIntRange`（`Hero.java` L699-705）
/// 同为伤害域 `NormalIntRange`。
///
/// 与 Java 的已知差异：`heroDamageIntRange` 先掷一次 `Float()` 与三叶草
/// 重掷概率比较（无该饰品时概率 0 恒不触发，但**消耗一掷**）；饰品属 M4，
/// 纯核不掷该次，随机流较 Java 每次少一掷、取值分布不变。
#[must_use]
pub fn damage_roll(stats: &CharStats, rng: &mut impl Rng) -> i32 {
    normal_int_range(rng, stats.damage_range.min, stats.damage_range.max)
}

/// 护甲减免掷值：护甲域上的 `NormalIntRange`（Rat：`Rat.java` L64-67 的
/// `(0, 1)`；Crab：`Crab.java` L55-58 的 `(0, 4)`；Snake 与裸身英雄无覆写，
/// 护甲域 `(0, 0)` 必得 0）。
///
/// 与 Java 的已知差异：`Char.drRoll`（L706-712）恒含一次 Barkskin 项
/// `NormalIntRange(0, Barkskin.currentLevel(…))`——无该 Buff 时上界为 0
/// （`Barkskin.java` L119-125）仍消耗两次 `Float()`。Buff 属 M4，纯核不掷
/// 该项，随机流较 Java 每次少两掷、取值不变（0 + x = x）。
#[must_use]
pub fn dr_roll(stats: &CharStats, rng: &mut impl Rng) -> i32 {
    normal_int_range(rng, stats.armor_range.min, stats.armor_range.max)
}

#[cfg(test)]
mod tests {
    use core::convert::Infallible;

    use rand::{SeedableRng, TryRng};

    use super::*;
    use crate::actors::bestiary::MobKind;

    /// 脚本化随机源：按给定 u32 字序出数，供逐位手算对拍。
    ///
    /// rand 0.10 将 u32 映射为 f32 `[0, 1)` 的规则是 `(word >> 8) / 2^24`
    /// （rand-0.10.2 `src/distr/float.rs`，precision = 24），下方
    /// [`word_to_f32_mapping_is_pinned`] 钉住该映射——rand 升级若改动实现，
    /// 先在该测试报警，手算用例的前提即失效。
    struct ScriptedRng {
        words: Vec<u32>,
        next: usize,
    }

    impl ScriptedRng {
        fn new(words: &[u32]) -> Self {
            Self {
                words: words.to_vec(),
                next: 0,
            }
        }

        /// 尚未消耗的字数——断言"该路径消耗/不消耗随机数"用。
        fn remaining(&self) -> usize {
            self.words.len() - self.next
        }
    }

    impl TryRng for ScriptedRng {
        type Error = Infallible;

        fn try_next_u32(&mut self) -> Result<u32, Infallible> {
            // 越界 panic 即测试脚本供字不足，是测试自身的缺陷
            let word = self.words[self.next];
            self.next += 1;
            Ok(word)
        }

        fn try_next_u64(&mut self) -> Result<u64, Infallible> {
            let lo = self.try_next_u32()?;
            let hi = self.try_next_u32()?;
            Ok(u64::from(lo) | (u64::from(hi) << 32))
        }

        fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Infallible> {
            for chunk in dst.chunks_mut(4) {
                let bytes = self.try_next_u32()?.to_le_bytes();
                chunk.copy_from_slice(&bytes[..chunk.len()]);
            }
            Ok(())
        }
    }

    /// `(w >> 8) / 2^24 = 0.0`。
    const W_ZERO: u32 = 0x0000_0000;
    /// `(w >> 8) / 2^24 = 2⁻²⁴`（最小非零掷值）。
    const W_MIN_POS: u32 = 0x0000_0100;
    /// `(w >> 8) / 2^24 = 0.25`。
    const W_QUARTER: u32 = 0x4000_0000;
    /// `(w >> 8) / 2^24 = 0.5`。
    const W_HALF: u32 = 0x8000_0000;
    /// `(w >> 8) / 2^24 = (2^24 - 1) / 2^24 = 1 - 2⁻²⁴`（最大掷值，恒 < 1）。
    const W_MAX: u32 = 0xFFFF_FFFF;

    /// 钉住 rand 0.10 的 u32 → f32 映射（手算用例的前提），并验证
    /// `Random.Float()` 的半开区间语义：可取 0，最大值严格小于 1
    /// （`Random.java` L76-79 注释 "[0, 1)"）。
    #[test]
    fn word_to_f32_mapping_is_pinned() {
        let mut rng = ScriptedRng::new(&[W_ZERO, W_MIN_POS, W_QUARTER, W_HALF, W_MAX]);
        assert_eq!(float01(&mut rng), 0.0);
        assert_eq!(float01(&mut rng), 2f32.powi(-24));
        assert_eq!(float01(&mut rng), 0.25);
        assert_eq!(float01(&mut rng), 0.5);
        let max = float01(&mut rng);
        assert_eq!(max, 16_777_215.0 / 16_777_216.0);
        assert!(max < 1.0, "Float() 永不取 1（半开区间）");
    }

    /// 命中手算对拍（Char.java L651/L665/L667/L683）。
    /// 攻 10 守 5、accMulti = 1：
    /// - 字 [0.5, 1-2⁻²⁴]：acuRoll = 0.5×10×1 = 5.0；
    ///   defRoll = (1-2⁻²⁴)×5 = 4.99999970… < 5.0 → 命中（L683 `>=` 成立）；
    /// - 字 [0.0, 2⁻²⁴]：acuRoll = 0；defRoll = 2⁻²⁴×5 ≈ 2.98e-7 > 0 → 未中；
    /// - 攻 10 守 10、字 [0.5, 0.5]：5.0 >= 5.0 平手 → 命中（`>=` 而非 `>`）。
    #[test]
    fn hit_hand_calc_matches_java_formula() {
        let mut rng = ScriptedRng::new(&[W_HALF, W_MAX]);
        assert!(hit_with_skills(10.0, 5.0, 1.0, &mut rng));
        assert_eq!(rng.remaining(), 0, "掷值路径恰好消耗两掷（先攻后守）");

        let mut rng = ScriptedRng::new(&[W_ZERO, W_MIN_POS]);
        assert!(!hit_with_skills(10.0, 5.0, 1.0, &mut rng));

        let mut rng = ScriptedRng::new(&[W_HALF, W_HALF]);
        assert!(
            hit_with_skills(10.0, 10.0, 1.0, &mut rng),
            "平手判中（L683 >=）"
        );
    }

    /// 掷序钉死：先掷攻方（L651）后掷守方（L667）。攻 10 守 10、
    /// 字 [0.0, 0.5]：acuRoll = 0、defRoll = 5 → 未中；若掷序颠倒则
    /// acuRoll = 5、defRoll = 0 → 必中——结果相反，可区分。
    #[test]
    fn hit_rolls_attacker_first() {
        let mut rng = ScriptedRng::new(&[W_ZERO, W_HALF]);
        assert!(!hit_with_skills(10.0, 10.0, 1.0, &mut rng));
    }

    /// `acc_multi` 乘在攻方掷值上（L665）：攻 10 守 6、字 [0.25, 0.5]。
    /// - accMulti = 1：acuRoll = 2.5 < defRoll = 3.0 → 未中；
    /// - accMulti = 2（魔法命中的便捷重载值，L620-622）：acuRoll = 0.25×10×2
    ///   = 5.0 >= 3.0 → 命中。
    ///
    /// 又：`accMulti` **不参与**无限命中阈值判定（L646 比较的是 `acuStat`）——
    /// 999 999 × 1000 仍走掷值路径（消耗两掷），1 000 000 直接判中（零掷）。
    #[test]
    fn acc_multi_scales_roll_not_threshold() {
        let mut rng = ScriptedRng::new(&[W_QUARTER, W_HALF]);
        assert!(!hit_with_skills(10.0, 6.0, 1.0, &mut rng));

        let mut rng = ScriptedRng::new(&[W_QUARTER, W_HALF]);
        assert!(hit_with_skills(10.0, 6.0, 2.0, &mut rng));

        let mut rng = ScriptedRng::new(&[W_ZERO, W_MAX]);
        assert!(!hit_with_skills(
            999_999.0,
            1_000_000.0 - 1.0,
            1000.0,
            &mut rng
        ));
        assert_eq!(rng.remaining(), 0, "阈值之下仍走掷值路径");

        let mut rng = ScriptedRng::new(&[W_ZERO, W_MAX]);
        assert!(hit_with_skills(1_000_000.0, 5.0, 1.0, &mut rng));
        assert_eq!(rng.remaining(), 2, "无限命中不消耗随机数（L646-649）");
    }

    /// 无限闪避压过无限命中（L641-645）：双方同为 1 000 000 时判未中，
    /// 且不消耗随机数。
    #[test]
    fn infinite_evasion_beats_infinite_accuracy() {
        let mut rng = ScriptedRng::new(&[W_MAX, W_MAX]);
        assert!(!hit_with_skills(1_000_000.0, 1_000_000.0, 1.0, &mut rng));
        assert_eq!(rng.remaining(), 2);
    }

    /// acc = 0 边界：`Float(0) = Float()×0 = 0`（Random.java L87-89），
    /// 仍消耗掷值。守方掷出严格正值（概率 1 - 2⁻²⁴）→ 未中；守方恰掷 0
    /// （字 `W_ZERO`）→ 0 >= 0 平手判中——与 Java 完全一致的极小概率命中。
    #[test]
    fn zero_accuracy_boundary() {
        let mut rng = ScriptedRng::new(&[W_MAX, W_MIN_POS]);
        assert!(!hit_with_skills(0.0, 5.0, 1.0, &mut rng));
        assert_eq!(rng.remaining(), 0, "acc=0 仍消耗两掷");

        let mut rng = ScriptedRng::new(&[W_MAX, W_ZERO]);
        assert!(
            hit_with_skills(0.0, 5.0, 1.0, &mut rng),
            "守方掷 0 时平手判中"
        );
    }

    /// evasion = 0 边界：defRoll = Float()×0 = 0，acuRoll >= 0 恒真 → 必中，
    /// 与掷出的字无关（但仍消耗两掷）。acc = 0 与 evasion = 0 同时成立时
    /// 0 >= 0 → 命中。
    #[test]
    fn zero_evasion_boundary() {
        let mut rng = ScriptedRng::new(&[W_ZERO, W_MAX]);
        assert!(hit_with_skills(5.0, 0.0, 1.0, &mut rng));

        let mut rng = ScriptedRng::new(&[W_MIN_POS, W_MIN_POS]);
        assert!(hit_with_skills(5.0, 0.0, 1.0, &mut rng));

        let mut rng = ScriptedRng::new(&[W_MAX, W_MAX]);
        assert!(
            hit_with_skills(0.0, 0.0, 1.0, &mut rng),
            "acc=0 vs eva=0 平手判中"
        );
        assert_eq!(rng.remaining(), 0);
    }

    /// [`hit`] 组件入口与标量形态一致（Rat 攻 8 对 Crab 守 5，字 [0.5, 0.5]：
    /// acuRoll = 4.0、defRoll = 2.5 → 命中）。
    #[test]
    fn hit_reads_component_skills() {
        let rat = MobKind::Rat.stats().char_stats;
        let crab = MobKind::Crab.stats().char_stats;
        let mut rng = ScriptedRng::new(&[W_HALF, W_HALF]);
        assert!(hit(&rat, &crab, 1.0, &mut rng));
        let mut rng = ScriptedRng::new(&[W_HALF, W_HALF]);
        assert_eq!(
            hit(&rat, &crab, 1.0, &mut rng),
            hit_with_skills(8.0, 5.0, 1.0, &mut ScriptedRng::new(&[W_HALF, W_HALF])),
        );
    }

    /// `NormalIntRange` 手算对拍（Random.java L138-140），Rat 伤害域 1-4
    /// （`Rat.java` L54-57）：
    /// - 字 [0.5, 0.5]：`1 + (int)((0.5+0.5)×(4-1+1)/2) = 1 + (int)2.0 = 3`；
    /// - 字 [0, 0]：下界 1；
    /// - 字 [1-2⁻²⁴, 1-2⁻²⁴]：和 = 2-2⁻²³（f32 精确表示），×4 = 8-2⁻²¹，
    ///   ÷2 = 4-2⁻²² = 3.99999976… → 截断 3 → 1+3 = **4**，闭区间上界可达。
    #[test]
    fn damage_roll_hand_calc_rat() {
        let rat = MobKind::Rat.stats().char_stats;

        let mut rng = ScriptedRng::new(&[W_HALF, W_HALF]);
        assert_eq!(damage_roll(&rat, &mut rng), 3);
        assert_eq!(rng.remaining(), 0, "NormalIntRange 恒消耗两掷");

        let mut rng = ScriptedRng::new(&[W_ZERO, W_ZERO]);
        assert_eq!(damage_roll(&rat, &mut rng), 1);

        let mut rng = ScriptedRng::new(&[W_MAX, W_MAX]);
        assert_eq!(damage_roll(&rat, &mut rng), 4);
    }

    /// f32 加法舍入与 Java `float` 一致的手算样例，Crab 伤害域 1-7
    /// （`Crab.java` L45-48）、字 [0.5, 1-2⁻²⁴]：
    /// 和 = 1.5 - 2⁻²⁴，恰处 f32 相邻可表示值 (1.5 - 2⁻²³, 1.5) 正中，
    /// IEEE-754 半偶舍入到 1.5（Java float 加法同规则）；
    /// 1 + (int)(1.5×7/2) = 1 + (int)5.25 = 6。
    #[test]
    fn damage_roll_hand_calc_crab_rounding() {
        let crab = MobKind::Crab.stats().char_stats;
        let mut rng = ScriptedRng::new(&[W_HALF, W_MAX]);
        assert_eq!(damage_roll(&crab, &mut rng), 6);
    }

    /// `drRoll` 手算对拍：
    /// - Crab 护甲域 0-4（`Crab.java` L55-58）、字 [0.25, 0.5]：
    ///   `0 + (int)((0.25+0.5)×(4-0+1)/2) = (int)1.875 = 1`；
    /// - Rat 护甲域 0-1（`Rat.java` L64-67）、字 [1-2⁻²⁴ ×2]：
    ///   `(int)((2-2⁻²³)×2/2) = (int)1.9999999 = 1` → 上界可达；
    /// - Snake 无覆写（`Snake.java` 全文无 `drRoll`）→ 域 (0,0) 必得 0，
    ///   但仍消耗两掷（对齐 `NormalIntRange(0,0)` 语义，区别于
    ///   `Random.Int(0)` 的零消耗）。
    #[test]
    fn dr_roll_hand_calc() {
        let crab = MobKind::Crab.stats().char_stats;
        let mut rng = ScriptedRng::new(&[W_QUARTER, W_HALF]);
        assert_eq!(dr_roll(&crab, &mut rng), 1);

        let rat = MobKind::Rat.stats().char_stats;
        let mut rng = ScriptedRng::new(&[W_MAX, W_MAX]);
        assert_eq!(dr_roll(&rat, &mut rng), 1);

        let snake = MobKind::Snake.stats().char_stats;
        let mut rng = ScriptedRng::new(&[W_HALF, W_HALF]);
        assert_eq!(dr_roll(&snake, &mut rng), 0);
        assert_eq!(rng.remaining(), 0, "域为 (0,0) 仍消耗两掷");
    }

    /// 固定种子确定性 + 值域性质（多种子）：同种子全序列一致；
    /// 伤害/护甲恒落在闭区间内且两端可达（三角分布不改变支撑集）。
    #[test]
    fn seeded_runs_are_deterministic_and_in_bounds() {
        let run = |seed: u64| {
            let mut rng = rand::rngs::ChaCha12Rng::seed_from_u64(seed);
            let rat = MobKind::Rat.stats().char_stats;
            let crab = MobKind::Crab.stats().char_stats;
            (0..64)
                .map(|_| {
                    (
                        hit(&rat, &crab, 1.0, &mut rng),
                        damage_roll(&crab, &mut rng),
                        dr_roll(&crab, &mut rng),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(run(0xDEAD_BEEF), run(0xDEAD_BEEF));

        for seed in 0..8_u64 {
            let mut rng = rand::rngs::ChaCha12Rng::seed_from_u64(seed);
            let crab = MobKind::Crab.stats().char_stats;
            let (mut saw_min, mut saw_max) = (false, false);
            for _ in 0..2000 {
                let dmg = damage_roll(&crab, &mut rng);
                assert!((1..=7).contains(&dmg), "伤害越界：{dmg}");
                saw_min |= dmg == 1;
                saw_max |= dmg == 7;
                let dr = dr_roll(&crab, &mut rng);
                assert!((0..=4).contains(&dr), "护甲越界：{dr}");
            }
            assert!(
                saw_min && saw_max,
                "种子 {seed}：2000 掷内两端点应各至少出现一次"
            );
        }
    }
}
