using System;

namespace Guildbound.Contracts
{
    public enum ItemCategory { Objective, Preparation, Equipment, Keepsake }

    public struct InventoryItem
    {
        public string Id, Name, Description, IconId;
        public ItemCategory Category;
        public int Count, MaxStack;
        public bool Equipped;
        public bool IsEmpty => string.IsNullOrEmpty(Id);
    }

    // What the inventory panel shows (AMEND-INV-01, H). Rules own every change; the panel only asks.
    public interface IInventoryModel
    {
        int SlotCount { get; }
        InventoryItem GetSlot(int index);
        bool CanUse(int index, out string reason);     // reason is shown when false
        void RequestUse(int index);
        bool CanEquip(int index, out string reason);
        void RequestEquip(int index);
        event Action Changed;
    }
}
