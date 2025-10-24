using System.Collections.Generic;

namespace LAsOsuBeatmapParser.Database
{
    /// <summary>
    /// Interface for objects that have files.
    /// </summary>
    public interface IHasFiles
    {
        /// <summary>
        /// The files associated with this object.
        /// </summary>
        IEnumerable<INamedFileUsage> Files { get; }

        /// <summary>
        /// Creates a file usage for the given filename.
        /// </summary>
        /// <param name="filename">The filename.</param>
        /// <returns>The file usage.</returns>
        INamedFileUsage CreateFileUsage(string filename);
    }

    /// <summary>
    /// Represents a named file usage.
    /// </summary>
    public interface INamedFileUsage
    {
        /// <summary>
        /// The filename.
        /// </summary>
        string Filename { get; }

        /// <summary>
        /// The file info.
        /// </summary>
        IFileInfo File { get; }
    }

    /// <summary>
    /// Represents file information.
    /// </summary>
    public interface IFileInfo
    {
        /// <summary>
        /// The hash of the file.
        /// </summary>
        string Hash { get; }

        /// <summary>
        /// The size of the file.
        /// </summary>
        long Size { get; }
    }
}
