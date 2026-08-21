//! SPD `Random.java` 语义的等价工具（`SPD-classes/.../watabou/utils/Random.java`），
//! 物品域私有实现——`levels` 域有同语义工具但按域边界不跨域 import。
//!
//! 与 SPD 的静态生成器栈不同，一律显式传 `&mut impl Rng`（docs/plans/01 · 确定性）。
//! Java `Random.pushGenerator(seed)`（`Random.java` L51-L53）对应本域用
//! [`ItemRng::seed_from_u64`](rand::SeedableRng::seed_from_u64) 新建私有流。
//! 注意：只保证**本工程内**种子确定性，不与 `java.util.Random` 位流对齐。

use rand::{Rng, RngExt};

/// 物品域统一随机源（ChaCha12：`seed_from_u64` 跨平台/跨版本稳定）。
pub type ItemRng = rand::rngs::ChaCha12Rng;

/// `Random.Float()`：均匀 `[0, 1)`（`Random.java` L77-L79）。
pub fn float(rng: &mut impl Rng) -> f32 {
    rng.random::<f32>()
}

/// `Random.Int(max)`：均匀 `[0, max)`；`max <= 0` 时返回 0（L114-L124）。
pub fn int(rng: &mut impl Rng, max: i32) -> i32 {
    if max <= 0 {
        0
    } else {
        rng.random_range(0..max)
    }
}

/// `Random.IntRange(min, max)`：均匀闭区间 `[min, max]`（L132-L134）。
pub fn int_range(rng: &mut impl Rng, min: i32, max: i32) -> i32 {
    min + int(rng, max - min + 1)
}

/// `Random.Long()` 的种子用法等价物：取一个 `u64` 作为私有流种子
/// （`Generator.java` L632/L713、`Weapon.java` L434 的 `Random.Long()` 场景）。
pub fn next_seed(rng: &mut impl Rng) -> u64 {
    rng.random::<u64>()
}

/// `Random.chances(float[])`：按权重返回下标；负权重按 0 计；
/// 权重和 ≤ 0 时**不消耗随机数**直接返回 `None`（对应 Java 的 -1，L175-L198）。
///
/// `Random.chances(HashMap)`（L202-L229）在 `LinkedHashMap` 插入序下与本函数
/// 语义一致（权重非负时），`Generator` 的类目牌堆按枚举序数组复用本函数。
pub fn chances(rng: &mut impl Rng, weights: &[f32]) -> Option<usize> {
    let sum: f32 = weights.iter().map(|w| w.max(0.0)).sum();
    if sum <= 0.0 {
        return None;
    }
    let value = float(rng) * sum;
    let mut acc = 0.0;
    for (i, w) in weights.iter().enumerate() {
        acc += w.max(0.0);
        if value < acc {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn rng(seed: u64) -> ItemRng {
        ItemRng::seed_from_u64(seed)
    }

    #[test]
    fn int_respects_bounds_and_degenerate_max() {
        let mut r = rng(1);
        assert_eq!(int(&mut r, 0), 0);
        assert_eq!(int(&mut r, -3), 0);
        for _ in 0..1000 {
            assert!((0..7).contains(&int(&mut r, 7)));
        }
    }

    #[test]
    fn int_range_is_inclusive_both_ends() {
        let mut r = rng(2);
        let mut seen = [false; 4];
        for _ in 0..1000 {
            let v = int_range(&mut r, 3, 6);
            assert!((3..=6).contains(&v));
            seen[(v - 3) as usize] = true;
        }
        assert!(seen.iter().all(|&s| s), "闭区间两端都应可取到");
    }

    #[test]
    fn chances_zero_sum_consumes_nothing() {
        // 权重和 ≤ 0 时不得消耗随机数（Random.java L184-L186 先判和后取 Float）：
        // 空牌堆判定不能扰动后续随机流。
        let mut a = rng(3);
        let mut b = rng(3);
        assert_eq!(chances(&mut a, &[0.0, 0.0]), None);
        assert_eq!(chances(&mut a, &[-1.0, -2.0]), None);
        assert_eq!(float(&mut a), float(&mut b), "None 分支后两条流仍应同步");
    }

    #[test]
    fn chances_respects_weights() {
        let mut r = rng(4);
        // 负权重按 0：只可能选到下标 1
        for _ in 0..100 {
            assert_eq!(chances(&mut r, &[-1.0, 5.0]), Some(1));
        }
        let mut seen = [false; 3];
        for _ in 0..1000 {
            seen[chances(&mut r, &[1.0, 3.0, 1.0]).unwrap()] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn seeded_stream_is_reproducible() {
        let mut a = rng(42);
        let mut b = rng(42);
        for _ in 0..32 {
            assert_eq!(float(&mut a), float(&mut b));
        }
        assert_eq!(next_seed(&mut a), next_seed(&mut b));
    }
}
