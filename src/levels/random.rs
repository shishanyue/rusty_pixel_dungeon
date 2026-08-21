//! SPD `Random.java` 语义的等价工具（`SPD-classes/.../watabou/utils/Random.java`）。
//!
//! 与 SPD 的静态生成器栈不同，这里一律显式传 `&mut impl Rng`
//! （见 docs/plans/01 · 确定性）；随机源用 ChaCha（rand 0.10 `chacha` 特性），
//! `seed_from_u64` 跨平台/跨版本稳定。注意：只保证**本工程内**种子确定性，
//! 不与 Java `java.util.Random` 的位流对齐。

use rand::{Rng, RngExt};

/// 关卡生成统一随机源。
pub type LevelRng = rand::rngs::ChaCha12Rng;

/// `Random.Float()`：均匀 `[0, 1)`（L77-L79）。
pub fn float(rng: &mut impl Rng) -> f32 {
    rng.random::<f32>()
}

/// `Random.Float(min, max)`：均匀 `[min, max)`（L92-L94）。
pub fn float_range(rng: &mut impl Rng, min: f32, max: f32) -> f32 {
    min + float(rng) * (max - min)
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

/// `Random.NormalIntRange(min, max)`：三角分布闭区间 `[min, max]`，
/// 越靠中间越常见（L138-L140）。
pub fn normal_int_range(rng: &mut impl Rng, min: i32, max: i32) -> i32 {
    min + ((float(rng) + float(rng)) * (max - min + 1) as f32 / 2.0) as i32
}

/// `Random.chances(float[])`：按权重返回下标；负权重按 0 计；
/// 权重和 ≤ 0 时返回 `None`（对应 Java 的 -1，L175-L198）。
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

/// `Random.element(T[])`：等概率取一个元素（L240-L246）。
/// 与 Java 数组版一致，空切片会 panic（调用方保证非空）。
pub fn element<'a, T>(rng: &mut impl Rng, items: &'a [T]) -> &'a T {
    &items[int(rng, items.len() as i32) as usize]
}

/// `Random.shuffle(T[])`：SPD 的前向 Fisher-Yates（L271-L280）。
pub fn shuffle<T>(rng: &mut impl Rng, items: &mut [T]) {
    if items.is_empty() {
        return;
    }
    for i in 0..items.len() - 1 {
        // Java: j = Int(i, length) = i + Int(length - i)
        let j = i + int(rng, (items.len() - i) as i32) as usize;
        if j != i {
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn rng(seed: u64) -> LevelRng {
        LevelRng::seed_from_u64(seed)
    }

    #[test]
    fn int_respects_bounds_and_degenerate_max() {
        let mut r = rng(1);
        assert_eq!(int(&mut r, 0), 0);
        assert_eq!(int(&mut r, -5), 0);
        for _ in 0..1000 {
            let v = int(&mut r, 7);
            assert!((0..7).contains(&v));
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
    fn normal_int_range_stays_in_bounds() {
        let mut r = rng(3);
        for _ in 0..1000 {
            let v = normal_int_range(&mut r, 4, 10);
            assert!((4..=10).contains(&v));
        }
    }

    #[test]
    fn chances_handles_zero_sum_and_negative_weights() {
        let mut r = rng(4);
        assert_eq!(chances(&mut r, &[0.0, 0.0]), None);
        assert_eq!(chances(&mut r, &[-1.0, -2.0]), None);
        // 负权重按 0：只可能选到下标 1
        for _ in 0..100 {
            assert_eq!(chances(&mut r, &[-1.0, 5.0]), Some(1));
        }
        // 全部下标最终都能被选到
        let mut seen = [false; 3];
        for _ in 0..1000 {
            seen[chances(&mut r, &[1.0, 3.0, 1.0]).unwrap()] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn shuffle_is_deterministic_per_seed() {
        let mut a: Vec<i32> = (0..16).collect();
        let mut b: Vec<i32> = (0..16).collect();
        shuffle(&mut rng(42), &mut a);
        shuffle(&mut rng(42), &mut b);
        assert_eq!(a, b);
        let mut c: Vec<i32> = (0..16).collect();
        shuffle(&mut rng(43), &mut c);
        assert_ne!(a, c, "不同种子几乎必然产生不同排列");
    }
}
