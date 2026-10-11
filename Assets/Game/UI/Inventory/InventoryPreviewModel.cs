using System;
using Guildbound.Contracts;
using UnityEngine;

namespace Guildbound.UI
{
    // Sample inventory data. Production recovery, appearance and saves belong to the rules model.
    public sealed class InventoryPreviewModel : MonoBehaviour, IInventoryModel
    {
        readonly InventoryItem[] slots = {
            new InventoryItem { Id = "seal", Name = "Sealed guild marker", Description = "A guild objective. Return it to the hall.\n\nOne carried objective at a time. It cannot be dropped or used.", IconId = "objective", Category = ItemCategory.Objective, Count = 1, MaxStack = 1 },
            new InventoryItem { Id = "prep", Name = "Preparation flask", Description = "Preparation for recovery outside combat.\n\nShortens recovery from 3 seconds to 1 second. Consumed only when recovery completes.", IconId = "preparation", Category = ItemCategory.Preparation, Count = 2, MaxStack = 2 },
            new InventoryItem { Id = "vest", Name = "Guild vest", Description = "An appearance piece for the dressing corner.\n\nChanges your look. Reach, damage and health stay the same.", IconId = "equipment", Category = ItemCategory.Equipment, Count = 1, MaxStack = 1 },
            new InventoryItem { Id = "route-record", Name = "Gate route keepsake", Description = "A record of a completed guild contract.\n\nKept as part of your story. It cannot be sold, dropped or used.", IconId = "keepsake", Category = ItemCategory.Keepsake, Count = 1, MaxStack = 1 },
            default, default, default, default, default, default, default, default
        };

        public int SlotCount => slots.Length;
        public InventoryItem GetSlot(int index) => slots[index];
        public event Action Changed;

        public bool CanUse(int index, out string reason)
        {
            reason = slots[index].Category == ItemCategory.Preparation && !slots[index].IsEmpty
                ? "Use through recovery outside combat."
                : "This item cannot be used.";
            return false;
        }

        public void RequestUse(int index) { } // No direct item-use action in this preview.

        public bool CanEquip(int index, out string reason)
        {
            bool allowed = !slots[index].IsEmpty && slots[index].Category == ItemCategory.Equipment;
            reason = allowed ? "" : "This item is not equipment.";
            return allowed;
        }

        public void RequestEquip(int index)
        {
            if (!CanEquip(index, out _)) return;
            var item = slots[index];
            item.Equipped = !item.Equipped;
            slots[index] = item;
            Changed?.Invoke();
        }
    }
}
