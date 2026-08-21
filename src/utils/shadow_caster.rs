//! SPD `ShadowCaster` 移植：递归阴影投射视野（FOV）。
//! 逐行对照 `core/src/main/java/com/shatteredpixel/shatteredpixeldungeon/mechanics/ShadowCaster.java`
//! （SPD 3.3.8），注释中的行号均指该文件。算法出处见 Java 头注释：
//! roguebasin 的 "FOV using recursive shadowcasting"。
//!
//! # 形态约定
//!
//! - 地图为线性数组，`index = y * width + x`；`los_blocking` 语义的遮挡表由
//!   调用方以 `&[bool]` 传入（对应 SPD `Level.updateFieldOfView` 里的 `blocking`）。
//! - Java 用 try/catch 兜底数组越界（L60-L72）：视野越出地图数组时整张 FOV 清空。
//!   本移植在扫描中做显式界检查并返回错误，效果一致。SPD 关卡四周恒为
//!   LOS 遮挡的实心墙，正常输入不会触发。
//! - Java 每次 `scanOctant` 重取（distance == 2 时克隆并修补）`rounding` 行
//!   （L86-L95）；本移植在 [`cast_shadow`] 里取一次传入递归，值恒等，仅省去重复克隆。

use std::sync::LazyLock;

/// 视野半径上限（L30）。
pub const MAX_DISTANCE: i32 = 20;

/// 圆形半径修正表（L32-L46）：`rounding[i][j]` 是视距 `i` 下 FOV 第 `j` 行的最大行长，
/// 把方形 FOV 修成圆形。测的是格子中心，故正弦用 `j / (i + 0.5)`（L40-L43）。
/// `rounding[i][0]` 与 Java 一样保持 0，不参与扫描（行号从 1 起）。
static ROUNDING: LazyLock<Vec<Vec<i32>>> = LazyLock::new(|| {
    let mut rounding = vec![Vec::new(); (MAX_DISTANCE + 1) as usize];
    for (i, row_out) in rounding.iter_mut().enumerate().skip(1) {
        let mut row = vec![0; i + 1];
        for (j, cap) in row.iter_mut().enumerate().skip(1) {
            let i_f = i as f64;
            let j_f = j as f64;
            // Java `Math.round` 对非负数等价于四舍五入；表内各值距 .5 边界的
            // 最小裕度约 3.4e-3（用 Java 实测），libm 间的 ulp 级差异不可能翻转结果
            let circle = (i_f * (j_f / (i_f + 0.5)).asin().cos()).round() as i64;
            *cap = (j as i64).min(circle) as i32;
        }
        *row_out = row;
    }
    rounding
});

/// 只读访问 `rounding` 表（Java 公有静态字段，L34），供消费方与对拍测试使用。
pub fn rounding_table() -> &'static [Vec<i32>] {
    &ROUNDING
}

/// 扫描越出地图数组（Java 里表现为 `ArrayIndexOutOfBoundsException` 被 L69-L72 捕获）。
struct OutOfBounds;

/// 对照 `castShadow`（L48-L74）：以 (`x`, `y`) 为源向 `field_of_view` 写入视野。
/// `distance` 超过 [`MAX_DISTANCE`] 时被截断（L50-L52）；源格恒可见（L57）。
/// 与 Java 一致：扫描中越出数组界（含 `distance` 为负）时整张 FOV 清空返回。
pub fn cast_shadow(
    x: i32,
    y: i32,
    width: i32,
    field_of_view: &mut [bool],
    blocking: &[bool],
    distance: i32,
) {
    assert_eq!(field_of_view.len(), blocking.len(), "两张表长度必须一致");
    assert!(width > 0, "width 必须为正");

    let distance = distance.min(MAX_DISTANCE);

    field_of_view.fill(false);

    // 源格置真（L57）
    field_of_view[(y * width + x) as usize] = true;

    // Java 在 scanOctant 里索引 rounding[distance]（L94），distance 为负时
    // 抛异常被 catch（L69-L72）——此处等价地清空返回
    let Some(rounding_row) = usize::try_from(distance).ok().and_then(|d| ROUNDING.get(d)) else {
        field_of_view.fill(false);
        return;
    };

    // 视距为 2 时补上视野四角：去角虽然在几何上正确，但会过度惩罚对角移动（L87-L92）
    let patched;
    let rounding_at_dist: &[i32] = if distance == 2 {
        let mut row = rounding_row.clone();
        row[2] = 2;
        patched = row;
        &patched
    } else {
        rounding_row
    };

    // 八个卦限，顺时针扫描（L60-L68）
    const OCTANTS: [(i32, i32, bool); 8] = [
        (1, -1, false),
        (-1, 1, true),
        (1, 1, true),
        (1, 1, false),
        (-1, 1, false),
        (1, -1, true),
        (-1, -1, true),
        (-1, -1, false),
    ];
    for (m_x, m_y, m_xy) in OCTANTS {
        let scan = scan_octant(
            distance,
            field_of_view,
            blocking,
            rounding_at_dist,
            1,
            x,
            y,
            width,
            0.0,
            1.0,
            m_x,
            m_y,
            m_xy,
        );
        if scan.is_err() {
            // Java catch：任一卦限越界则整张 FOV 清空（L69-L72）
            field_of_view.fill(false);
            return;
        }
    }
}

/// 对照 `scanOctant`（L76-L164）：扫描一个 45° 卦限，
/// 通过 X 镜像（`m_x`）、Y 镜像（`m_y`）与 X=Y 对换（`m_xy`）拼出整个 FOV。
/// 所有计算相对源格中心偏移 0.5（L97）。
fn scan_octant(
    distance: i32,
    fov: &mut [bool],
    blocking: &[bool],
    rounding_at_dist: &[i32],
    mut row: i32,
    x: i32,
    y: i32,
    w: i32,
    mut l_slope: f64,
    r_slope: f64,
    m_x: i32,
    m_y: i32,
    m_xy: bool,
) -> Result<(), OutOfBounds> {
    let size = fov.len() as i32;
    let mut in_blocking = false;

    // 逐行扫描，从当前行起（L100）
    while row <= distance {
        // 扫描区间为负，直接结束（L102-L103）
        if r_slope < l_slope {
            return Ok(());
        }

        // 偏移取略小于 0.5 的 0.499，容忍斜率恰好擦过格角（L105-L111）
        let start = if l_slope == 0.0 {
            0
        } else {
            ((f64::from(row) - 0.5) * l_slope + 0.499).floor() as i32
        };
        let end = if r_slope == 1.0 {
            rounding_at_dist[row as usize]
        } else {
            rounding_at_dist[row as usize]
                .min(((f64::from(row) + 0.5) * r_slope - 0.499).ceil() as i32)
        };

        // 源格坐标（L113-L114）
        let mut cell = x + y * w;

        // 叠加当前格坐标（含 X、Y、X=Y 三种镜像，L116-L118）
        if m_xy {
            cell += m_x * start * w + m_y * row;
        } else {
            cell += m_x * start + m_y * row * w;
        }

        let mut col = start;
        while col <= end {
            // 处理行末斜率比行首多推进一格、且前一格遮挡的误差情形（L124-L127）
            if col == end
                && in_blocking
                && ((f64::from(row) - 0.5) * r_slope - 0.499).ceil() as i32 != end
            {
                break;
            }

            // Java 直接索引 fov[cell]（L130），越界抛异常由 castShadow 捕获
            if cell < 0 || cell >= size {
                return Err(OutOfBounds);
            }
            let idx = cell as usize;

            fov[idx] = true;

            if blocking[idx] {
                if !in_blocking {
                    in_blocking = true;

                    // 另起一趟深一行的扫描，右界收到当前格左侧（L136-L142）
                    if col != start {
                        scan_octant(
                            distance,
                            fov,
                            blocking,
                            rounding_at_dist,
                            row + 1,
                            x,
                            y,
                            w,
                            l_slope,
                            // Δx / Δy（L139-L140）
                            (f64::from(col) - 0.5) / (f64::from(row) + 0.5),
                            m_x,
                            m_y,
                            m_xy,
                        )?;
                    }
                }
            } else if in_blocking {
                in_blocking = false;

                // 后续行的扫描左界收到当前格左侧；斜率为 Δx / Δy（L146-L152）
                l_slope = (f64::from(col) - 0.5) / (f64::from(row) - 0.5);
            }

            if m_xy {
                cell += m_x * w;
            } else {
                cell += m_x;
            }

            col += 1;
        }

        // 行末仍在遮挡中，本趟扫描结束（L161-L162）
        if in_blocking {
            return Ok(());
        }

        row += 1;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `'1'` 可见 / `'0'` 不可见 / `'#'` 遮挡（仅 blocking 表用）。
    fn parse_grid(rows: &[&str], truthy: u8) -> Vec<bool> {
        let width = rows[0].len();
        let mut grid = Vec::with_capacity(width * rows.len());
        for row in rows {
            assert_eq!(row.len(), width);
            grid.extend(row.bytes().map(|b| b == truthy));
        }
        grid
    }

    fn render(fov: &[bool], width: usize) -> Vec<String> {
        fov.chunks(width)
            .map(|row| row.iter().map(|&v| if v { '1' } else { '0' }).collect())
            .collect()
    }

    /// 空场 FOV 的期望形状：对每个格子取 a = max(|dx|, |dy|)（卦限行号）、
    /// b = min(|dx|, |dy|)（行内列号），可见当且仅当 a == 0，
    /// 或 a ≤ distance 且 b ≤ 行长上限（`rounding` 表 + L87-L92 的 distance == 2 补角）。
    fn expected_empty_fov(width: i32, src_x: i32, src_y: i32, distance: i32) -> Vec<bool> {
        let mut caps = rounding_table()[distance as usize].clone();
        if distance == 2 {
            caps[2] = 2;
        }
        let mut fov = vec![false; (width * width) as usize];
        for cy in 0..width {
            for cx in 0..width {
                let dx = (cx - src_x).abs();
                let dy = (cy - src_y).abs();
                let (a, b) = (dx.max(dy), dx.min(dy));
                if a == 0 || (a <= distance && b <= caps[a as usize]) {
                    fov[(cy * width + cx) as usize] = true;
                }
            }
        }
        fov
    }

    /// `rounding` 表逐值对拍逐字复刻的 Java 静态初始化块（L35-L46）在本机的输出。
    #[test]
    fn rounding_table_matches_java() {
        #[rustfmt::skip]
        let expected: [&[i32]; 21] = [
            &[],
            &[1],
            &[1, 1],
            &[1, 2, 2],
            &[1, 2, 3, 2],
            &[1, 2, 3, 3, 2],
            &[1, 2, 3, 4, 4, 2],
            &[1, 2, 3, 4, 5, 4, 3],
            &[1, 2, 3, 4, 5, 6, 5, 3],
            &[1, 2, 3, 4, 5, 6, 6, 5, 3],
            &[1, 2, 3, 4, 5, 6, 7, 6, 5, 3],
            &[1, 2, 3, 4, 5, 6, 7, 8, 7, 5, 3],
            &[1, 2, 3, 4, 5, 6, 7, 8, 8, 7, 6, 3],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 9, 8, 6, 4],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 9, 8, 6, 4],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 9, 8, 6, 4],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 11, 10, 8, 7, 4],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 11, 10, 9, 7, 4],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 12, 11, 9, 7, 4],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 13, 12, 11, 9, 7, 4],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 14, 13, 11, 10, 8, 4],
        ];

        let table = rounding_table();
        assert_eq!(table.len(), (MAX_DISTANCE + 1) as usize);
        assert!(table[0].is_empty());
        for i in 1..=MAX_DISTANCE as usize {
            assert_eq!(table[i].len(), i + 1);
            assert_eq!(
                table[i][0], 0,
                "行长上限从 j = 1 起，j = 0 与 Java 同为默认 0"
            );
            assert_eq!(&table[i][1..], expected[i], "rounding[{i}] 与 Java 不一致");
        }
    }

    /// 空场圆形边界逐格对拍：全图逐格对照由 `rounding` 表导出的圆形谓词，
    /// 并以逐字复刻 Java `castShadow` 的实际网格输出为锚
    /// （distance = 2 验证 L87-L92 的补角：5×5 满方块）。
    #[test]
    fn empty_room_fov_matches_rounding_circle() {
        let width = 25usize;
        let blocking = vec![false; width * width];
        let mut fov = vec![false; width * width];

        for distance in 1..=8 {
            cast_shadow(12, 12, width as i32, &mut fov, &blocking, distance);
            assert_eq!(
                render(&fov, width),
                render(&expected_empty_fov(width as i32, 12, 12, distance), width),
                "distance = {distance} 的空场 FOV 与 rounding 表圆形不一致"
            );
        }

        // Java 网格锚点：distance = 2（补角后 5×5 满方块）
        cast_shadow(12, 12, width as i32, &mut fov, &blocking, 2);
        let band: Vec<String> = render(&fov, width)[10..=14].to_vec();
        assert_eq!(band, vec!["0000000000111110000000000"; 5]);

        // Java 网格锚点：distance = 6 的中心 13 行
        cast_shadow(12, 12, width as i32, &mut fov, &blocking, 6);
        let expected_band = [
            "0000000000111110000000000",
            "0000000011111111100000000",
            "0000000111111111110000000",
            "0000000111111111110000000",
            "0000001111111111111000000",
            "0000001111111111111000000",
            "0000001111111111111000000",
            "0000001111111111111000000",
            "0000001111111111111000000",
            "0000000111111111110000000",
            "0000000111111111110000000",
            "0000000011111111100000000",
            "0000000000111110000000000",
        ];
        let rendered = render(&fov, width);
        assert_eq!(&rendered[6..=18], &expected_band);
        for (y, row) in rendered.iter().enumerate() {
            if !(6..=18).contains(&y) {
                assert_eq!(
                    row,
                    &"0".repeat(width),
                    "distance = 6 时第 {y} 行应全不可见"
                );
            }
        }
    }

    /// 柱子遮挡的阴影锥：源 (10, 10)、柱 (10, 8)、视距 6，
    /// 全图逐格对拍逐字复刻 Java `castShadow` 的网格输出。
    /// 柱后阴影：dy = -3、-4、-5 行挡住正上一格，dy = -6 行挡三格；
    /// 柱下方半圆完整不受影响。
    #[test]
    fn pillar_casts_shadow_cone() {
        let width = 21usize;
        let mut blocking = vec![false; width * width];
        blocking[8 * width + 10] = true;
        let mut fov = vec![false; width * width];

        cast_shadow(10, 10, width as i32, &mut fov, &blocking, 6);

        let expected = [
            "000000000000000000000",
            "000000000000000000000",
            "000000000000000000000",
            "000000000000000000000",
            "000000001000100000000",
            "000000111101111000000",
            "000001111101111100000",
            "000001111101111100000",
            "000011111111111110000",
            "000011111111111110000",
            "000011111111111110000",
            "000011111111111110000",
            "000011111111111110000",
            "000001111111111100000",
            "000001111111111100000",
            "000000111111111000000",
            "000000001111100000000",
            "000000000000000000000",
            "000000000000000000000",
            "000000000000000000000",
            "000000000000000000000",
        ];
        assert_eq!(render(&fov, width), expected);
    }

    /// 源四周 8 格全遮挡：可见的恰是源与整圈遮挡格（3×3 块），
    /// 对拍 Java 网格输出；覆盖 L124-L127 行末遮挡守卫的不中断分支。
    #[test]
    fn enclosed_source_sees_only_adjacent_ring() {
        let width = 9usize;
        let blocking = parse_grid(
            &[
                "000000000",
                "000000000",
                "000000000",
                "000###000",
                "000#0#000",
                "000###000",
                "000000000",
                "000000000",
                "000000000",
            ],
            b'#',
        );
        let mut fov = vec![false; width * width];

        cast_shadow(4, 4, width as i32, &mut fov, &blocking, 6);

        let expected = [
            "000000000",
            "000000000",
            "000000000",
            "000111000",
            "000111000",
            "000111000",
            "000000000",
            "000000000",
            "000000000",
        ];
        assert_eq!(render(&fov, width), expected);
    }

    /// 视距 0 只见源格（行循环不执行，对拍 Java 输出）；
    /// 视距 ≥ `MAX_DISTANCE` 时按 L50-L52 截断，25 与 20 的输出全同。
    #[test]
    fn distance_zero_and_max_clamp() {
        let width = 9usize;
        let blocking = vec![false; width * width];
        let mut fov = vec![false; width * width];
        cast_shadow(4, 4, width as i32, &mut fov, &blocking, 0);
        let mut expected = vec![false; width * width];
        expected[4 * width + 4] = true;
        assert_eq!(fov, expected);

        let width = 45usize;
        let blocking = vec![false; width * width];
        let mut fov20 = vec![false; width * width];
        let mut fov25 = vec![false; width * width];
        cast_shadow(22, 22, width as i32, &mut fov20, &blocking, 20);
        cast_shadow(22, 22, width as i32, &mut fov25, &blocking, 25);
        assert_eq!(fov20, fov25);
        assert_eq!(
            render(&fov20, width),
            render(&expected_empty_fov(width as i32, 22, 22, 20), width)
        );
    }
}
