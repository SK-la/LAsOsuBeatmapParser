using System;

namespace LAsOsuBeatmapParser.Analysis
{
    /// <summary>
    /// 交叉矩阵提供者，用于SR计算中的列间权重矩阵
    /// </summary>
    public static class CrossMatrixProvider
    {
        /// <summary>
        /// 获取指定键数(K)的交叉矩阵
        /// K表示键数，从1开始索引
        /// </summary>
        /// <param name="K">键数</param>
        /// <returns>交叉矩阵数组</returns>
        public static double[] GetMatrix(int K)
        {
            if (K < 1 || K > CrossMatrices.Length)
                throw new ArgumentOutOfRangeException(nameof(K), $"不支持的键数: {K}，支持范围: 1-{CrossMatrices.Length}");

            return CrossMatrices[K - 1];
        }

        /// <summary>
        /// 检查指定的键数是否受支持
        /// </summary>
        /// <param name="K">键数</param>
        /// <returns>是否受支持</returns>
        public static bool IsSupported(int K)
        {
            return K >= 1 && K <= CrossMatrices.Length && CrossMatrices[K - 1] != null;
        }

        /// <summary>
        /// 获取所有支持的键数
        /// </summary>
        /// <returns>支持的键数数组</returns>
        public static int[] GetSupportedKeys()
        {
            var supported = new System.Collections.Generic.List<int>();
            for (int i = 0; i < CrossMatrices.Length; i++)
            {
                if (CrossMatrices[i] != null)
                    supported.Add(i + 1);
            }
            return supported.ToArray();
        }

        /// <summary>
        /// 交叉矩阵数据，表示各键位两侧的权重分布
        /// 索引0对应K=1，索引1对应K=2，以此类推
        /// null表示不支持该键数
        /// </summary>
        private static readonly double[][] CrossMatrices =
        [
            // K=1
            [0.075, 0.075],
            // K=2
            [0.125, 0.05, 0.125],
            // K=3
            [0.125, 0.125, 0.125, 0.125],
            // K=4
            [0.175, 0.25, 0.05, 0.25, 0.175],
            // K=5
            [0.175, 0.25, 0.175, 0.175, 0.25, 0.175],
            // K=6
            [0.225, 0.35, 0.25, 0.05, 0.25, 0.35, 0.225],
            // K=7
            [0.225, 0.35, 0.25, 0.225, 0.225, 0.25, 0.35, 0.225],
            // K=8
            [0.275, 0.45, 0.35, 0.25, 0.05, 0.25, 0.35, 0.45, 0.275],
            // K=9
            [0.275, 0.45, 0.35, 0.25, 0.275, 0.275, 0.25, 0.35, 0.45, 0.275],
            // K=10
            [0.625, 0.55, 0.45, 0.35, 0.25, 0.05, 0.25, 0.35, 0.45, 0.55, 0.625],
            // K=11 (odd, unsupported)
            null,
            // K=12
            [0.8, 0.8, 0.8, 0.6, 0.4, 0.2, 0.05, 0.2, 0.4, 0.6, 0.8, 0.8, 0.8],
            // K=13 (odd, unsupported)
            null,
            // K=14
            [0.4, 0.4, 0.2, 0.2, 0.3, 0.3, 0.1, 0.1, 0.3, 0.3, 0.2, 0.2, 0.4, 0.4, 0.4],
            // K=15 (odd, unsupported)
            null,
            // K=16
            [0.4, 0.4, 0.2, 0.2, 0.4, 0.4, 0.2, 0.1, 0.1, 0.2, 0.4, 0.4, 0.2, 0.2, 0.4, 0.4, 0.4],
            // K=17 (odd, unsupported)
            null,
            // K=18
            [0.4, 0.4, 0.2, 0.4, 0.2, 0.4, 0.2, 0.3, 0.1, 0.1, 0.3, 0.2, 0.4, 0.2, 0.4, 0.2, 0.4, 0.4, 0.4]
        ];
    }
}
