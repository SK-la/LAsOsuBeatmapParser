// 示例：如何在代码中处理未使用警告

using System;

namespace LAsOsuBeatmapParser.Beatmaps
{
    /// <summary>
    /// 示例类展示如何处理库中的未使用警告
    /// </summary>
    public class ExampleClass
    {
        // 方法1：使用#pragma warning disable来局部禁用警告
        #pragma warning disable CS0169 // 禁用"Field is never used"警告
        private int _unusedField;
        #pragma warning restore CS0169 // 恢复警告

        // 方法2：对于公共API，使用下划线前缀表示故意未使用
        public void PublicMethod(int _parameter)
        {
            // 参数前面加下划线表示故意未使用
            Console.WriteLine("This method is part of public API");
        }

        // 方法3：如果安装了JetBrains.Annotations包，可以使用[UsedImplicitly]
        // [UsedImplicitly]
        // public string PublicProperty { get; set; }

        // 方法4：对于确实未使用的私有成员，可以删除或重构
        // 如果必须保留（比如序列化），使用#pragma warning
    }
}
