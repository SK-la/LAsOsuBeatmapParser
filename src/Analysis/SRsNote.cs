using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Threading.Tasks;
using LAsOsuBeatmapParser.Beatmaps;
using System.Runtime.InteropServices;
using System.Text.Json;

namespace LAsOsuBeatmapParser.Analysis
{
    /// <summary>
    /// SR算法专用Note类
    /// </summary>
    public class SRsNote
    {
        /// <summary>
        /// Key mode
        /// </summary>
        public int Index { get; set; }

        /// <summary>
        /// from start time of note
        /// </summary>
        public int StartTime { get; set; }

        /// <summary>
        /// from end time of tail, -1 if not LN
        /// </summary>
        public int EndTime { get; set; }

        /// <summary>
        /// 构造函数
        /// </summary>
        /// <param name="index"></param>
        /// <param name="startTime"></param>
        /// <param name="endTime"></param>
        public SRsNote(int index, int startTime, int endTime)
        {
            Index = index;
            StartTime = startTime;
            EndTime = endTime;
        }
    }

    /// <summary>
    /// 快速排序比较器，先按StartTime排序，若相同则按Index排序
    /// </summary>
    public class NoteComparer : IComparer<SRsNote>
    {
        public int Compare(SRsNote x, SRsNote y)
        {
            int result = x.StartTime.CompareTo(y.StartTime);
            if (result == 0)
            {
                result = x.Index.CompareTo(y.Index);
            }
            return result;
        }
    }

    /// <summary>
    /// note比较器，按EndTime排序
    /// </summary>
    public class NoteComparerByT : IComparer<SRsNote>
    {
        /// <summary>
        /// 比较函数
        /// </summary>
        /// <param name="a"></param>
        /// <param name="b"></param>
        /// <returns></returns>
        public int Compare(SRsNote? a, SRsNote? b)
        {
            if (a != null && b != null)
                return a.EndTime.CompareTo(b.EndTime);

            return 0;
        }
    }
}
