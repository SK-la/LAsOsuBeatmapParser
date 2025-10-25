using System;
using System.Collections;
using System.Collections.Generic;

namespace LAsOsuBeatmapParser.FakeFrameWork
{
    /// <summary>
    ///     A list that maintains its elements in sorted order.
    ///     Allows duplicates.
    /// </summary>
    /// <typeparam name="T">The type of elements in the list.</typeparam>
    public class SortedList<T> : IList<T>, IReadOnlyList<T>
        where T : IComparable<T>
    {
        private readonly List<T> list = new List<T>();

        /// <summary>
        ///     Gets the number of elements contained in the list.
        /// </summary>
        public int Count
        {
            get => list.Count;
        }

        /// <summary>
        ///     Gets a value indicating whether the list is read-only.
        /// </summary>
        public bool IsReadOnly
        {
            get => false;
        }

        /// <summary>
        ///     Gets or sets the element at the specified index.
        /// </summary>
        /// <param name="index">The zero-based index of the element to get or set.</param>
        /// <returns>The element at the specified index.</returns>
        public T this[int index]
        {
            get => list[index];
            set => list[index] = value;
            // Note: Setting an element may break sorting. For simplicity, we assume the user maintains order.
        }

        /// <summary>
        ///     Adds an item to the list in sorted order.
        /// </summary>
        /// <param name="item">The item to add.</param>
        public void Add(T item)
        {
            int index            = list.BinarySearch(item);
            if (index < 0) index = ~index;
            list.Insert(index, item);
        }

        /// <summary>
        ///     Removes all items from the list.
        /// </summary>
        public void Clear()
        {
            list.Clear();
        }

        /// <summary>
        ///     Determines whether the list contains a specific value.
        /// </summary>
        /// <param name="item">The object to locate in the list.</param>
        /// <returns>true if item is found in the list; otherwise, false.</returns>
        public bool Contains(T item)
        {
            return list.Contains(item);
        }

        /// <summary>
        ///     Copies the elements of the list to an array, starting at a particular array index.
        /// </summary>
        /// <param name="array">The one-dimensional array that is the destination of the elements copied from the list.</param>
        /// <param name="arrayIndex">The zero-based index in array at which copying begins.</param>
        public void CopyTo(T[] array, int arrayIndex)
        {
            list.CopyTo(array, arrayIndex);
        }

        /// <summary>
        ///     Returns an enumerator that iterates through the collection.
        /// </summary>
        /// <returns>An enumerator that can be used to iterate through the collection.</returns>
        public IEnumerator<T> GetEnumerator()
        {
            return list.GetEnumerator();
        }

        /// <summary>
        ///     Determines the index of a specific item in the list.
        /// </summary>
        /// <param name="item">The object to locate in the list.</param>
        /// <returns>The index of item if found in the list; otherwise, -1.</returns>
        public int IndexOf(T item)
        {
            return list.IndexOf(item);
        }

        /// <summary>
        ///     Inserts an item to the list at the specified index. Note: This may break sorting.
        /// </summary>
        /// <param name="index">The zero-based index at which item should be inserted.</param>
        /// <param name="item">The object to insert into the list.</param>
        public void Insert(int index, T item)
        {
            list.Insert(index, item);
            // For simplicity, we don't re-sort here.
        }

        /// <summary>
        ///     Removes the first occurrence of a specific object from the list.
        /// </summary>
        /// <param name="item">The object to remove from the list.</param>
        /// <returns>true if item was successfully removed from the list; otherwise, false.</returns>
        public bool Remove(T item)
        {
            return list.Remove(item);
        }

        /// <summary>
        ///     Removes the element at the specified index of the list.
        /// </summary>
        /// <param name="index">The zero-based index of the element to remove.</param>
        public void RemoveAt(int index)
        {
            list.RemoveAt(index);
        }

        /// <summary>
        ///     Returns an enumerator that iterates through the collection.
        /// </summary>
        /// <returns>An enumerator that can be used to iterate through the collection.</returns>
        IEnumerator IEnumerable.GetEnumerator()
        {
            return GetEnumerator();
        }
    }
}
