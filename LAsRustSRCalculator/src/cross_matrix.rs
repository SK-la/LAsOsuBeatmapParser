pub struct CrossMatrixProvider;

impl CrossMatrixProvider {
    /// 获取指定键数(K)的交叉矩阵
    /// K表示键数，从1开始索引
    pub fn get_matrix(k: usize) -> Option<&'static [f64]> {
        if k < 1 || k > Self::MATRICES.len() {
            return None;
        }
        Self::MATRICES.get(k - 1).and_then(|opt| *opt)
    }

    /// 检查指定的键数是否受支持
    pub fn is_supported(k: usize) -> bool {
        k >= 1 && k <= Self::MATRICES.len() && Self::MATRICES[k - 1].is_some()
    }

    /// 获取所有支持的键数
    pub fn get_supported_keys() -> Vec<usize> {
        (0..Self::MATRICES.len())
            .filter(|&i| Self::MATRICES[i].is_some())
            .map(|i| i + 1)
            .collect()
    }

    /// 交叉矩阵数据
    /// 索引0对应K=1，索引1对应K=2，以此类推
    /// None表示不支持该键数
    const MATRICES: &'static [Option<&'static [f64]>] = &[
        // K=1
        Some(&[-1.0]),
        // K=2
        Some(&[0.075, 0.075]),
        // K=3
        Some(&[0.125, 0.05, 0.125]),
        // K=4
        Some(&[0.125, 0.125, 0.125, 0.125]),
        // K=5
        Some(&[0.175, 0.25, 0.05, 0.25, 0.175]),
        // K=6
        Some(&[0.175, 0.25, 0.175, 0.175, 0.25, 0.175]),
        // K=7
        Some(&[0.225, 0.35, 0.25, 0.05, 0.25, 0.35, 0.225]),
        // K=8
        Some(&[0.225, 0.35, 0.25, 0.225, 0.225, 0.25, 0.35, 0.225]),
        // K=9
        Some(&[0.275, 0.45, 0.35, 0.25, 0.05, 0.25, 0.35, 0.45, 0.275]),
        // K=10
        Some(&[0.275, 0.45, 0.35, 0.25, 0.275, 0.275, 0.25, 0.35, 0.45, 0.275]),
        // K=11 (odd, unsupported)
        None,
        // K=12 (even, sides 3 columns higher)
        Some(&[0.8, 0.8, 0.8, 0.6, 0.4, 0.2, 0.05, 0.2, 0.4, 0.6, 0.8, 0.8, 0.8]),
        // K=13 (odd, unsupported)
        None,
        // K=14 (wave: low-low-high-high-low-low-high-high)
        Some(&[0.4, 0.4, 0.2, 0.2, 0.3, 0.3, 0.1, 0.1, 0.3, 0.3, 0.2, 0.2, 0.4, 0.4, 0.4]),
        // K=15 (odd, unsupported)
        None,
        // K=16 (wave: low-low-high-high-low-low-high-high-low-low-high-high)
        Some(&[0.4, 0.4, 0.2, 0.2, 0.4, 0.4, 0.2, 0.1, 0.1, 0.2, 0.4, 0.4, 0.2, 0.2, 0.4, 0.4, 0.4]),
        // K=17 (odd, unsupported)
        None,
        // K=18 (wave: low-low-high-low-high-low-high-low-low-high-low-high-low-high)
        Some(&[0.4, 0.4, 0.2, 0.4, 0.2, 0.4, 0.2, 0.3, 0.1, 0.1, 0.3, 0.2, 0.4, 0.2, 0.4, 0.2, 0.4, 0.4, 0.4]),
    ];
}