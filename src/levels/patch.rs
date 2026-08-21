//! 细胞自动机噪声，对照 `core/.../levels/Patch.java`（水/草补丁的形状来源）。
//!
//! 算法三段（Patch.java L42-L143）：
//! 1. 按 `fill` 概率播种随机布尔图（`forceFillRate` 时先把 fill 向 0.5 拉半程，
//!    抵消聚簇对填充率的推挤）；
//! 2. `clustering` 趟 3×3 多数表决平滑（平票算 true），把散点聚成补丁；
//! 3. `forceFillRate` 时随机找已有补丁/空洞的边沿生长/收缩，
//!    直到填充数**恰好**等于 `round(w*h*fill)`。

use rand::Rng;

use crate::levels::{
    builder::java_round_f32,
    random::{float, int},
};

/// `Patch.generate(w, h, fill, clustering, forceFillRate)`（Patch.java L42-L143）。
/// 返回 `w*h` 布尔图（行优先，index = y*w + x），true 即"补丁内"。
///
/// `forceFillRate && min(w,h) > 2` 时，true 的个数恰为 `round(w*h*fill)`
/// （Java `Math.round`，即 `floor(x+0.5)`）。
pub fn generate(
    rng: &mut impl Rng,
    w: usize,
    h: usize,
    fill: f32,
    clustering: i32,
    force_fill_rate: bool,
) -> Vec<bool> {
    let length = w * h;

    let mut cur = vec![false; length];
    let mut off = vec![false; length];

    // L49：fillDiff 以**原始** fill 结算目标数（在 L51-L53 的调整之前）
    let mut fill_diff: i32 = -java_round_f32(length as f32 * fill);

    // L51-L53：聚簇会把填充率推向 0/1，先把种子填充率向 0.5 拉半程抵消
    let mut fill = fill;
    if force_fill_rate && clustering > 0 {
        fill += (0.5 - fill) * 0.5;
    }

    // L55-L58：播种
    for (i, cell) in off.iter_mut().enumerate() {
        let _ = i;
        *cell = float(rng) < fill;
        if *cell {
            fill_diff += 1;
        }
    }

    // L60-L115：3×3 多数表决（含自身，平票 true），逐趟在 cur/off 间乒乓
    for _ in 0..clustering {
        for y in 0..h {
            for x in 0..w {
                let pos = x + y * w;
                let mut count = 0;
                let mut neighbours = 0;

                if y > 0 {
                    if x > 0 {
                        if off[pos - w - 1] {
                            count += 1;
                        }
                        neighbours += 1;
                    }
                    if off[pos - w] {
                        count += 1;
                    }
                    neighbours += 1;
                    if x < w - 1 {
                        if off[pos - w + 1] {
                            count += 1;
                        }
                        neighbours += 1;
                    }
                }

                if x > 0 {
                    if off[pos - 1] {
                        count += 1;
                    }
                    neighbours += 1;
                }
                if off[pos] {
                    count += 1;
                }
                neighbours += 1;
                if x < w - 1 {
                    if off[pos + 1] {
                        count += 1;
                    }
                    neighbours += 1;
                }

                if y < h - 1 {
                    if x > 0 {
                        if off[pos + w - 1] {
                            count += 1;
                        }
                        neighbours += 1;
                    }
                    if off[pos + w] {
                        count += 1;
                    }
                    neighbours += 1;
                    if x < w - 1 {
                        if off[pos + w + 1] {
                            count += 1;
                        }
                        neighbours += 1;
                    }
                }

                cur[pos] = 2 * count >= neighbours;
                if cur[pos] != off[pos] {
                    fill_diff += if cur[pos] { 1 } else { -1 };
                }
            }
        }

        std::mem::swap(&mut cur, &mut off);
    }

    // L117-L140：精确校正填充数。只在有边界圈可用（min > 2）时做——
    // 生长点取自内圈 [1, w-2]×[1, h-2]，其 3×3 邻域不会越界。
    if force_fill_rate && w.min(h) > 2 {
        let wi = w as isize;
        let neighbours: [isize; 9] = [-wi - 1, -wi, -wi + 1, -1, 0, 1, wi - 1, wi, wi + 1];
        let growing = fill_diff < 0;

        // Java 无迭代上限：只要 fill_diff != 0 就必存在可转换格，
        // 且每次转换把 fill_diff 朝 0 推 1 格，概率意义下必然终止。
        while fill_diff != 0 {
            let mut cell;
            let mut tries: usize = 0;

            // L126-L131：随机取非边界格；先尽量找已在补丁/空洞内的格子
            // （从它生长而不是另起新补丁），length/10 次找不到就将就。
            // `Random.Int(1, w-1)` 上界开区间 → 1 + Int(w-2)。
            loop {
                cell = (1 + int(rng, w as i32 - 2)) as usize
                    + (1 + int(rng, h as i32 - 2)) as usize * w;
                tries += 1;
                if off[cell] == growing || tries * 10 >= length {
                    break;
                }
            }

            for &n in &neighbours {
                let i = (cell as isize + n) as usize;
                if fill_diff != 0 && off[i] != growing {
                    off[i] = growing;
                    fill_diff += if growing { 1 } else { -1 };
                }
            }
        }
    }

    off
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels::random::LevelRng;
    use rand::SeedableRng;

    fn ascii(patch: &[bool], w: usize) -> String {
        let mut out = String::new();
        for row in patch.chunks(w) {
            for &b in row {
                out.push(if b { '#' } else { '.' });
            }
            out.push('\n');
        }
        out
    }

    /// 同种子同参数逐格一致（对拍基础）。
    #[test]
    fn same_seed_same_shape() {
        for seed in [0u64, 7, 42, 9999] {
            let a = generate(&mut LevelRng::seed_from_u64(seed), 20, 15, 0.3, 5, true);
            let b = generate(&mut LevelRng::seed_from_u64(seed), 20, 15, 0.3, 5, true);
            assert_eq!(a, b, "seed {seed}");
        }
    }

    /// 钉死用例：seed=7、12×8、fill=0.3、clustering=2、forceFillRate。
    /// 期望图由本实现首次生成后人工核对锁定（ChaCha12 种子稳定，跨平台不变）；
    /// 填充数 = round(96*0.3) = 29，且成片聚簇而非散点。
    #[test]
    fn pinned_shape_seed_7() {
        let patch = generate(&mut LevelRng::seed_from_u64(7), 12, 8, 0.3, 2, true);
        let expected = "\
######......
######......
####........
####........
##..........
#...........
##......#...
##......#...
";
        assert_eq!(
            ascii(&patch, 12),
            expected,
            "实际形状：\n{}",
            ascii(&patch, 12)
        );
    }

    /// forceFillRate 的硬保证：true 数恰为 `round(len*fill)`（Patch.java L117-L140）。
    #[test]
    fn forced_fill_rate_is_exact() {
        let mut rng = LevelRng::seed_from_u64(123);
        for (w, h, fill, clustering) in [
            (32usize, 32usize, 0.30f32, 5),
            (20, 15, 0.20, 4),
            (33, 17, 0.85, 5),
            (40, 12, 0.80, 4),
            (9, 9, 0.45, 1),
        ] {
            let patch = generate(&mut rng, w, h, fill, clustering, true);
            let expected = java_round_f32((w * h) as f32 * fill);
            let actual = patch.iter().filter(|&&b| b).count() as i32;
            assert_eq!(
                actual, expected,
                "{w}x{h} fill={fill} clustering={clustering}：填充数必须精确"
            );
        }
    }

    /// 不强制填充率、无聚簇时就是伯努利播种：均值落在 fill 邻域（统计断言）。
    #[test]
    fn unforced_seeding_matches_fill_statistically() {
        let mut rng = LevelRng::seed_from_u64(5);
        let patch = generate(&mut rng, 100, 100, 0.3, 0, false);
        let rate = patch.iter().filter(|&&b| b).count() as f32 / 10_000.0;
        // 10000 格伯努利 p=0.3 的标准差 ≈ 0.0046，±0.03 是 6.5σ 的宽裕带
        assert!(
            (rate - 0.3).abs() < 0.03,
            "无校正播种率应接近 fill，得 {rate}"
        );
    }

    /// 聚簇把散点聚成补丁：平滑后"孤立 true"（8 邻域全 false）应大幅减少。
    #[test]
    fn clustering_reduces_isolated_cells() {
        let isolated = |patch: &[bool], w: usize, h: usize| -> usize {
            let mut n = 0;
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    let i = y * w + x;
                    if patch[i]
                        && !patch[i - 1]
                        && !patch[i + 1]
                        && !patch[i - w]
                        && !patch[i + w]
                        && !patch[i - w - 1]
                        && !patch[i - w + 1]
                        && !patch[i + w - 1]
                        && !patch[i + w + 1]
                    {
                        n += 1;
                    }
                }
            }
            n
        };
        let raw = generate(&mut LevelRng::seed_from_u64(9), 60, 60, 0.3, 0, false);
        let smooth = generate(&mut LevelRng::seed_from_u64(9), 60, 60, 0.3, 5, true);
        assert!(
            isolated(&smooth, 60, 60) < isolated(&raw, 60, 60).max(1),
            "聚簇后孤立点应少于原始播种"
        );
    }
}
