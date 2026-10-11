using System.Collections.Generic;
using Guildbound.Contracts;
using Guildbound.Rules;
using NUnit.Framework;

namespace Guildbound.Tests
{
    // AMEND-INV-01 inventory rules. These are edit-mode only: the rules are pure C# with no
    // MonoBehaviour and no scene, so they are asserted directly.
    public class InventoryFixtures
    {
        sealed class Context : IInventoryContext
        {
            public string Objective;
            public bool Combat;
            public readonly List<DeliveredContract> DeliveredList = new List<DeliveredContract>();

            public string CarriedObjective => Objective;
            public IReadOnlyList<DeliveredContract> Delivered => DeliveredList;
            public bool InCombat => Combat;
        }

        static InventoryModel New(out Context context)
        {
            context = new Context();
            return new InventoryModel(context);
        }

        static InventoryModel New()
        {
            return New(out _);
        }

        [Test]
        public void SlotCount_IsTwelve()
        {
            var model = New();
            Assert.AreEqual(12, model.SlotCount);
            Assert.AreEqual(InventoryModel.TotalSlots, model.SlotCount);
        }

        [Test]
        public void ObjectiveSlot_ReadsTheContext_AndNeverKeepsACopy()
        {
            var model = New(out var ctx);
            Assert.IsTrue(model.GetSlot(0).IsEmpty);

            ctx.Objective = "Tool crate";
            Assert.AreEqual("Tool crate", model.GetSlot(0).Name);
            Assert.AreEqual(ItemCategory.Objective, model.GetSlot(0).Category);

            ctx.Objective = null;
            Assert.IsTrue(model.GetSlot(0).IsEmpty, "the model must not cache the objective it does not own");
        }

        [Test]
        public void Objective_CannotBeUsedOrWorn()
        {
            var model = New(out var ctx);
            ctx.Objective = "Tool crate";

            Assert.IsFalse(model.CanUse(0, out var useReason));
            Assert.AreEqual(InventoryModel.ObjectiveUseRefusal, useReason);

            Assert.IsFalse(model.CanEquip(0, out var wearReason));
            Assert.AreEqual(InventoryModel.ObjectiveWearRefusal, wearReason);
        }

        [Test]
        public void Preparation_StacksToCap_AndIsSpentOnlyOnCompletion()
        {
            var model = New();

            Assert.AreEqual(0, model.PreparationCount);
            model.GrantPreparation(5);
            Assert.AreEqual(InventoryModel.PreparationStackCap, model.PreparationCount);
            Assert.AreEqual(InventoryModel.PreparationStackCap, model.GetSlot(1).Count);
            Assert.AreEqual(ItemCategory.Preparation, model.GetSlot(1).Category);

            Assert.IsTrue(model.CanUse(1, out _));
            model.RequestUse(1);
            Assert.IsTrue(model.PreparationRequested);
            Assert.AreEqual(InventoryModel.PreparationStackCap, model.PreparationCount, "asking must not spend");

            Assert.IsTrue(model.ConsumePreparation());
            Assert.IsFalse(model.PreparationRequested);
            Assert.AreEqual(InventoryModel.PreparationStackCap - 1, model.PreparationCount);
        }

        [Test]
        public void Preparation_IsRefusedInCombat()
        {
            var model = New(out var ctx);
            model.GrantPreparation(1);
            ctx.Combat = true;

            Assert.IsFalse(model.CanUse(1, out var reason));
            Assert.AreEqual(InventoryModel.InCombatRefusal, reason);

            int raised = 0;
            model.Changed += () => raised++;
            model.RequestUse(1);
            Assert.AreEqual(0, raised, "a refused request must not raise Changed");
        }

        [Test]
        public void Garments_EquipOneAtATime_AndTogglingTakesItOff()
        {
            var model = New();
            model.AddGarment("g1", "Worn coat", "Cosmetic only.", "garment");
            model.AddGarment("g2", "Guild sash", "Cosmetic only.", "garment");

            Assert.IsFalse(model.GetSlot(2).Equipped);
            model.RequestEquip(2);
            Assert.IsTrue(model.GetSlot(2).Equipped);

            model.RequestEquip(3);
            Assert.IsTrue(model.GetSlot(3).Equipped);
            Assert.IsFalse(model.GetSlot(2).Equipped, "equipping a second garment must take the first off");

            model.RequestEquip(3);
            Assert.IsFalse(model.GetSlot(3).Equipped, "clicking the worn garment takes it off");
        }

        [Test]
        public void Keepsakes_ComeFromDeliveredContracts_AndAreDisplayOnly()
        {
            var model = New(out var ctx);
            ctx.DeliveredList.Add(new DeliveredContract { Id = "c1", Title = "Recover the tool crate." });

            Assert.AreEqual("Recover the tool crate.", model.GetSlot(6).Name);
            Assert.AreEqual(ItemCategory.Keepsake, model.GetSlot(6).Category);

            Assert.IsFalse(model.CanUse(6, out var useReason));
            Assert.AreEqual(InventoryModel.KeepsakeUseRefusal, useReason);

            Assert.IsFalse(model.CanEquip(6, out var wearReason));
            Assert.AreEqual(InventoryModel.KeepsakeWearRefusal, wearReason);
        }

        [Test]
        public void EmptyAndOutOfRangeSlots_RefuseSafely_AndExplainThemselves()
        {
            var model = New();

            for (int i = 0; i < model.SlotCount; i++)
            {
                Assert.IsTrue(model.GetSlot(i).IsEmpty, "slot " + i);
                Assert.IsFalse(model.CanUse(i, out var reason), "slot " + i);
                Assert.IsFalse(string.IsNullOrEmpty(reason), "slot " + i + " must give a reason");
            }

            Assert.IsTrue(model.GetSlot(-1).IsEmpty);
            Assert.IsTrue(model.GetSlot(model.SlotCount).IsEmpty);

            Assert.IsFalse(model.CanUse(99, out var outOfRange));
            Assert.AreEqual(InventoryModel.EmptySlotRefusal, outOfRange);

            model.RequestUse(99);
            model.RequestEquip(-5);
        }

        [Test]
        public void Changed_IsRaisedOnEveryMutation()
        {
            var model = New();
            int raised = 0;
            model.Changed += () => raised++;

            model.GrantPreparation(1);
            Assert.AreEqual(1, raised);

            model.RequestUse(1);
            Assert.AreEqual(2, raised);

            model.ConsumePreparation();
            Assert.AreEqual(3, raised);

            model.AddGarment("g1", "Worn coat", "Cosmetic only.", "garment");
            Assert.AreEqual(4, raised);

            model.RequestEquip(2);
            Assert.AreEqual(5, raised);
        }
    }
}
