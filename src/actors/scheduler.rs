//! 回合时间轮纯逻辑核，逐行对照 SPD `actors/Actor.java`（下文行号均指该文件）。
//!
//! 零 Bevy 依赖：本层只处理 `(id, time, priority)` 三元组与 `now` 时钟标量，
//! ECS 适配（组件/资源/事件分发）见同域 `turn` 模块。单测全部打在本层。

/// 一个标准回合的时间跨度（Actor.java L39 `TICK`）。
pub const TICK: f32 = 1.0;

/// 视觉特效优先级（Actor.java L48）：`time` 相同时最先行动。
pub const VFX_PRIO: i32 = 100;
/// 英雄优先级（Actor.java L49）：正值在英雄前，负值在英雄后。
pub const HERO_PRIO: i32 = 0;
/// Blob（气体等）优先级（Actor.java L50）：英雄之后、怪物之前。
pub const BLOB_PRIO: i32 = -10;
/// 怪物优先级（Actor.java L51）。
pub const MOB_PRIO: i32 = -20;
/// Buff 优先级（Actor.java L52）：一回合内最后行动的常规类别。
pub const BUFF_PRIO: i32 = -30;
/// 未指定优先级的兜底值（Actor.java L53 `DEFAULT`）：在所有类别之后。
pub const DEFAULT_PRIO: i32 = -100;

/// SPD 的防漂移取整（Actor.java L63-67，`postpone` 内 L82-86 重复同一逻辑）：
/// 若 `time` 与整数的偏差满足 `|time % 1| < 0.001`，取整到最近整数。
///
/// 注意这是**单侧**判定：只捕获略高于整数的值（如 5.0004 → 5.0）；略低于整数的
/// 值（如 2.9999998，`|x % 1|` ≈ 0.9999998）不会被取整——与 Java 原文一致，
/// 勿"好心修正"（Rust `%` 与 Java `%` 同为符号随被除数的 fmod）。
///
/// 与 Java 的唯一已知差异：`Math.round(float)` 返回 `int`，会把极大值饱和到
/// `i32::MAX`；Rust `f32::round` 保值。仅当 `time ≈ f32::MAX`（失活语义）时可
/// 观察到，Rust 行为（失活者保持失活）更合理，详见计划文档实现笔记。
#[must_use]
pub fn round_near_whole(time: f32) -> f32 {
    let ex = (time % 1.0).abs();
    if ex < 0.001 { time.round() } else { time }
}

/// `spendConstant`（Actor.java L61-68）：`time += amount` 后做防漂移取整。
/// M1 尚无时间修饰因子（冰冻/加速在 M4 的 Char 层），`spend`（Java L71-73
/// 仅转调 `spendConstant`）与之等价，故只提供这一个入口。
#[must_use]
pub fn spend(time: f32, amount: f32) -> f32 {
    round_near_whole(time + amount)
}

/// `postpone`（Actor.java L79-88）：把 `time` 推迟到不早于 `now + delay`，随后
/// 防漂移取整。只向后推：`time` 已在目标时刻之后（含恰好相等，L80 严格 `<`）
/// 则原样保留，绝不回拨。
#[must_use]
pub fn postpone(time: f32, now: f32, delay: f32) -> f32 {
    if time < now + delay {
        round_near_whole(now + delay)
    } else {
        time
    }
}

/// [`select_next`] 的选择结果：下一行动者及其行动时刻。
/// 调用方须以 `now = time` 推进时钟（Actor.java L271 `now = current.time`）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Selected<I> {
    /// 被选中的行动者。
    pub id: I,
    /// 该行动者的 `time`，即推进后的新 `now`。
    pub time: f32,
}

/// 下一行动者选择（Actor.java `process()` 选择循环 L253-266）：取 `time` 最小者；
/// `time` 相同取 `priority` 更大者（L259-260，方向见 L55 注释 "Higher values act
/// earlier"）。比较用严格 `>`，完全平手时保留先遍历到的——Java 遍历 `HashSet`
/// 顺序不确定，这里钉死为输入迭代序（Bevy 层即同原型内的生成序），属确定性
/// 增强而非语义偏离。失活者（`time = f32::MAX`）没有更早者时同样会被选中，
/// 与 Java 一致（L253 `earliest` 初值即 `MAX_VALUE`）。
pub fn select_next<I>(candidates: impl IntoIterator<Item = (I, f32, i32)>) -> Option<Selected<I>> {
    let mut best: Option<(Selected<I>, i32)> = None;
    for (id, time, priority) in candidates {
        let replace = match &best {
            None => true,
            Some((sel, best_prio)) => {
                time < sel.time || (time == sel.time && priority > *best_prio)
            }
        };
        if replace {
            best = Some((Selected { id, time }, priority));
        }
    }
    best.map(|(sel, _)| sel)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 最小时间轮：按 SPD `process()` 语义反复"选人 → 推钟 → 行动"
    /// （Actor.java L269-299 中与线程/精灵无关的骨架）。
    struct Wheel {
        now: f32,
        /// `(id, time, priority)`；`Vec` 序即遍历序。
        actors: Vec<(u32, f32, i32)>,
    }

    impl Wheel {
        /// 一次迭代：选中者花费 `cost`，返回其 id。
        fn step(&mut self, cost: f32) -> u32 {
            let sel = select_next(self.actors.iter().copied()).expect("时间轮不应为空");
            self.now = sel.time; // Actor.java L271
            let entry = self
                .actors
                .iter_mut()
                .find(|(id, ..)| *id == sel.id)
                .expect("选中者必然在轮上");
            entry.1 = spend(entry.1, cost);
            sel.id
        }
    }

    /// 验收 a：三个不同 time/priority 的轮转顺序。
    /// 对拍 Actor.java：常量表 L48-53；tie-break 方向 L55（"Higher values act
    /// earlier"）+ 选择比较 L259-260（`actor.actPriority > current.actPriority`）；
    /// 时钟推进 L271。
    #[test]
    fn rotation_order_matches_spd() {
        let mut wheel = Wheel {
            now: 0.0,
            actors: vec![
                (1, 1.0, HERO_PRIO), // 英雄
                (2, 1.0, MOB_PRIO),  // 怪物：与英雄同 time，priority 更小 → 靠后
                (3, 0.5, BUFF_PRIO), // Buff：time 最早 → 最先（priority 此时无关）
            ],
        };
        let mut order = Vec::new();
        let mut clocks = Vec::new();
        for _ in 0..6 {
            order.push(wheel.step(TICK));
            clocks.push(wheel.now);
        }
        // buff(0.5) → hero/mob 同 1.0 比 priority（0 > -20）→ buff(1.5) → 再一轮
        assert_eq!(order, [3, 1, 2, 3, 1, 2]);
        assert_eq!(clocks, [0.5, 1.0, 1.0, 1.5, 2.0, 2.0]);
    }

    /// VFX 在同 time 下先于英雄（Actor.java L48 注释 "visual effects take
    /// priority"，L49 "positive is before hero"）。
    #[test]
    fn vfx_acts_before_hero_on_equal_time() {
        let picked = select_next([(1, 1.0, HERO_PRIO), (2, 1.0, VFX_PRIO)]).unwrap();
        assert_eq!(picked.id, 2);
        assert_eq!(picked.time, 1.0);
    }

    /// time 与 priority 完全平手：保留先遍历到的（L260 严格 `>` 不替换）。
    /// Java 的 `HashSet` 遍历序不定；本移植钉死为输入序以获得确定性。
    #[test]
    fn full_tie_keeps_iteration_order() {
        let picked = select_next([(7, 2.0, DEFAULT_PRIO), (8, 2.0, DEFAULT_PRIO)]).unwrap();
        assert_eq!(picked.id, 7);
    }

    /// 失活（time = `f32::MAX`）没有更早者时仍会被选中（Actor.java L253/L259-260：
    /// `earliest` 初值 `MAX_VALUE`，`time == earliest && current == null` 命中）。
    #[test]
    fn deactivated_actor_still_selectable_when_alone() {
        let picked = select_next([(1, f32::MAX, DEFAULT_PRIO)]).unwrap();
        assert_eq!(picked.id, 1);
        assert_eq!(picked.time, f32::MAX);
        assert_eq!(select_next(Vec::<(u32, f32, i32)>::new()), None);
    }

    /// 验收 b：千次 spend(1/3) 后 now 不漂移（Actor.java L63-67 取整生效）。
    ///
    /// 无取整对照（IEEE f32 逐次累加实测）：终值误差 7.7e-4 且随步数线性增长；
    /// 取整版终值误差 1.0e-5（第 999 步恰被 snap 拉回整点 333.0）——阈值 1e-4
    /// 对两种实现有 7 倍以上的双向区分度。
    ///
    /// 如实说明：单侧取整下瞬时误差仍可短暂达 ~2e-3（第 768 步附近，Java 行为
    /// 完全相同），且时钟量级增大后 f32 ulp 逼近 0.001、snap 逐渐失效——SPD 靠
    /// `fixTime()` 定期把全体 time 拉回小量级兜底，该函数随 M4 的 `GameScene`
    /// 对应物一起移植。
    #[test]
    fn thousand_third_spends_do_not_drift() {
        let third = 1.0_f32 / 3.0;
        let mut wheel = Wheel {
            now: 0.0,
            actors: vec![(1, 0.0, HERO_PRIO)],
        };
        for _ in 0..1000 {
            wheel.step(third);
        }
        let time = wheel.actors[0].1;
        assert!(
            (f64::from(time) - 1000.0 / 3.0).abs() < 1e-4,
            "千次 spend 后 time={time} 偏离精确值 333.333…"
        );
        // 第 1000 次行动时选中者 time 为 999 次 spend 的累计 → now 回到整点 333.0
        assert!(
            (f64::from(wheel.now) - 333.0).abs() < 1e-4,
            "now={} 偏离 333.0",
            wheel.now
        );
    }

    /// 取整把"略高于整数"的累计误差吸掉：10 × spend(0.1) 恰为 1.0
    /// （f32 无取整时为 1.0000001，Actor.java L63-67 将其归整）。
    #[test]
    fn spend_snaps_slightly_above_whole_to_exact() {
        let mut t = 0.0_f32;
        for _ in 0..10 {
            t = spend(t, 0.1);
        }
        assert_eq!(t, 1.0);
    }

    /// 钉住 Java 单侧取整语义（Actor.java L64-65）：9 × spend(1/3) 得 2.9999998，
    /// 距 3 仅 ~2.4e-7，但 `|t % 1| ≈ 0.9999998` 不满足 `< 0.001`，**不**取整。
    #[test]
    fn rounding_is_one_sided_like_java() {
        let third = 1.0_f32 / 3.0;
        let mut t = 0.0_f32;
        for _ in 0..9 {
            t = spend(t, third);
        }
        assert!(t < 3.0, "落在整数下方时不应被取整（Java 同）");
        assert!(3.0 - t < 1e-6);
    }

    /// 验收 c：postpone 只向后推（Actor.java L79-88）。
    #[test]
    fn postpone_never_rewinds() {
        // 推迟：time 早于 now + delay → 设为 now + delay
        assert_eq!(postpone(5.0, 5.0, 2.0), 7.0);
        // 取整：0.5 + 0.5004 = 1.0004 → 1.0（L82-86 与 spend 同一取整）
        assert_eq!(postpone(0.2, 0.5, 0.5004), 1.0);
        // 不回拨：time 已在目标之后 → 原样
        assert_eq!(postpone(10.0, 5.0, 2.0), 10.0);
        // 恰好相等也不动（L80 严格 <，虽然结果值相同，语义上不重写）
        assert_eq!(postpone(7.0, 5.0, 2.0), 7.0);
    }

    /// spend 允许负值（Java `clearTime` L94-101 即 `spendConstant(-now)`）。
    #[test]
    fn spend_accepts_negative_amounts() {
        assert_eq!(spend(5.5, -5.5), 0.0);
        assert_eq!(spend(5.2, -0.2), 5.0);
    }
}
