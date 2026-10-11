using System;
using System.Collections.Generic;
using Guildbound.Contracts;

namespace Guildbound.Rules
{
    // What the inventory needs from the rest of the game, so it never keeps a second copy of state it does
    // not own. The Play layer implements this over GuildState (the carried objective and the delivered
    // contracts) and over the live combat world (the in-combat gate).
    public interface IInventoryContext
    {
        string CarriedObjective { get; }
        IReadOnlyList<DeliveredContract> Delivered { get; }
        bool InCombat { get; }
    }

    public struct DeliveredContract
    {
        public string Id;
        public string Title;
    }

    // AMEND-INV-01. Rules own every change; the panel only asks, and every refusal carries a reason the
    // HUD can show. Numbers marked (H) are tuning hypotheses, not verified results.
    public class InventoryModel : IInventoryModel
    {
        // (H) Twelve slots, partitioned by category: the objective keeps its single expedition slot, one
        // preparation stack, four dressing-corner garments, and the rest are keepsakes.
        public const int ObjectiveSlots = 1;
        public const int PreparationSlots = 1;
        public const int EquipmentSlots = 4;
        public const int KeepsakeSlots = 6;
        public const int TotalSlots = ObjectiveSlots + PreparationSlots + EquipmentSlots + KeepsakeSlots;
        public const int PreparationStackCap = 2;   // (H)

        public const string PreparationId = "prep.dressing";
        public const string PreparationName = "Field dressing";
        public const string PreparationEffect = "Shortens recovery to 1 s. Spent only on completion.";

        public const string EmptySlotRefusal = "Empty slot.";
        public const string ObjectiveUseRefusal = "The objective must be delivered, not used.";
        public const string ObjectiveWearRefusal = "The objective must be delivered, not worn.";
        public const string GarmentUseRefusal = "Garments are worn, not used.";
        public const string SupplyWearRefusal = "The dressing corner does not take supplies.";
        public const string KeepsakeUseRefusal = "Keepsakes are a record, not a tool.";
        public const string KeepsakeWearRefusal = "Keepsakes are a record, not equipment.";
        public const string InCombatRefusal = "Not during a fight.";
        public const string NoPreparationRefusal = "No preparation item.";

        readonly IInventoryContext context;
        readonly List<InventoryItem> garments = new List<InventoryItem>();
        int preparation;
        bool preparationRequested;

        public event Action Changed;

        public InventoryModel(IInventoryContext context)
        {
            this.context = context ?? throw new ArgumentNullException(nameof(context));
        }

        public int SlotCount => TotalSlots;

        // The authoritative preparation count. The combat driver copies this in and spends it on a
        // completed Recover; the panel never decrements it.
        public int PreparationCount => preparation;

        // True when the panel asked to use one. Spending happens on completion, never on request.
        public bool PreparationRequested => preparationRequested;

        public void GrantPreparation(int amount)
        {
            if (amount <= 0) return;
            preparation = Math.Min(PreparationStackCap, preparation + amount);
            Changed?.Invoke();
        }

        public bool ConsumePreparation()
        {
            if (preparation <= 0) return false;
            preparation--;
            preparationRequested = false;
            Changed?.Invoke();
            return true;
        }

        public void ClearPreparationRequest()
        {
            if (!preparationRequested) return;
            preparationRequested = false;
            Changed?.Invoke();
        }

        // Dressing-corner pieces. Cosmetic only: equipping changes look and build keys, never reach,
        // damage or HP (GDD: reach comes from the body, damage from stats).
        public void AddGarment(string id, string name, string description, string iconId)
        {
            if (garments.Count >= EquipmentSlots) return;
            garments.Add(new InventoryItem
            {
                Id = id,
                Name = name,
                Description = description,
                IconId = iconId,
                Category = ItemCategory.Equipment,
                Count = 1,
                MaxStack = 1
            });
            Changed?.Invoke();
        }

        public InventoryItem GetSlot(int index)
        {
            if (index < 0 || index >= TotalSlots) return default;

            if (index < ObjectiveSlots) return ObjectiveSlot();
            index -= ObjectiveSlots;

            if (index < PreparationSlots) return PreparationSlot();
            index -= PreparationSlots;

            if (index < EquipmentSlots) return index < garments.Count ? garments[index] : default;
            index -= EquipmentSlots;

            var delivered = context.Delivered;
            if (delivered == null || index >= delivered.Count) return default;
            var d = delivered[index];
            return new InventoryItem
            {
                Id = d.Id,
                Name = d.Title,
                Description = "A record of a contract you completed.",
                IconId = "keepsake",
                Category = ItemCategory.Keepsake,
                Count = 1,
                MaxStack = 1
            };
        }

        public bool CanUse(int index, out string reason)
        {
            reason = null;

            if (index < 0 || index >= TotalSlots) { reason = EmptySlotRefusal; return false; }

            if (index < ObjectiveSlots)
            {
                if (string.IsNullOrEmpty(context.CarriedObjective)) { reason = EmptySlotRefusal; return false; }
                reason = ObjectiveUseRefusal;
                return false;
            }
            index -= ObjectiveSlots;

            if (index < PreparationSlots)
            {
                if (preparation <= 0) { reason = NoPreparationRefusal; return false; }
                if (context.InCombat) { reason = InCombatRefusal; return false; }
                return true;
            }
            index -= PreparationSlots;

            if (index < EquipmentSlots)
            {
                if (index >= garments.Count) { reason = EmptySlotRefusal; return false; }
                reason = GarmentUseRefusal;
                return false;
            }
            index -= EquipmentSlots;

            var delivered = context.Delivered;
            if (delivered == null || index >= delivered.Count) { reason = EmptySlotRefusal; return false; }
            reason = KeepsakeUseRefusal;
            return false;
        }

        public void RequestUse(int index)
        {
            if (!CanUse(index, out _)) return;
            preparationRequested = true;
            Changed?.Invoke();
        }

        public bool CanEquip(int index, out string reason)
        {
            reason = null;

            if (index < 0 || index >= TotalSlots) { reason = EmptySlotRefusal; return false; }

            if (index < ObjectiveSlots)
            {
                if (string.IsNullOrEmpty(context.CarriedObjective)) { reason = EmptySlotRefusal; return false; }
                reason = ObjectiveWearRefusal;
                return false;
            }
            index -= ObjectiveSlots;

            if (index < PreparationSlots)
            {
                if (preparation <= 0) { reason = EmptySlotRefusal; return false; }
                reason = SupplyWearRefusal;
                return false;
            }
            index -= PreparationSlots;

            if (index < EquipmentSlots)
            {
                if (index >= garments.Count) { reason = EmptySlotRefusal; return false; }
                return true;
            }
            index -= EquipmentSlots;

            var delivered = context.Delivered;
            if (delivered == null || index >= delivered.Count) { reason = EmptySlotRefusal; return false; }
            reason = KeepsakeWearRefusal;
            return false;
        }

        public void RequestEquip(int index)
        {
            if (!CanEquip(index, out _)) return;

            int garment = index - ObjectiveSlots - PreparationSlots;
            bool wasWorn = garments[garment].Equipped;

            // One garment at a time: clicking the worn piece takes it off, any other piece swaps it in.
            for (int i = 0; i < garments.Count; i++)
            {
                var item = garments[i];
                item.Equipped = !wasWorn && i == garment;
                garments[i] = item;
            }
            Changed?.Invoke();
        }

        InventoryItem ObjectiveSlot()
        {
            string carried = context.CarriedObjective;
            if (string.IsNullOrEmpty(carried)) return default;
            return new InventoryItem
            {
                Id = "objective",
                Name = carried,
                Description = "Return it to the hall. It cannot be used, worn or moved.",
                IconId = "objective",
                Category = ItemCategory.Objective,
                Count = 1,
                MaxStack = 1
            };
        }

        InventoryItem PreparationSlot()
        {
            if (preparation <= 0) return default;
            return new InventoryItem
            {
                Id = PreparationId,
                Name = PreparationName,
                Description = PreparationEffect,
                IconId = "preparation",
                Category = ItemCategory.Preparation,
                Count = preparation,
                MaxStack = PreparationStackCap
            };
        }
    }
}
