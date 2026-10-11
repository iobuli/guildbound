using System.IO;
using Guildbound.Progress;
using NUnit.Framework;

namespace Guildbound.Tests
{
    // GDD v5.1 sections 5-7 test matrix: delivery once, replay pays nothing, abandon/defeat keep rewards, zero-wallet
    // retry, reload mid-attempt resets, interrupted save write recovers, attunement survives quitting right after.
    public class ProgressFixtures
    {
        string dir;

        [SetUp] public void SetUp() => dir = Path.Combine(Path.GetTempPath(), "guildbound-tests-" + System.Guid.NewGuid().ToString("N"));
        [TearDown] public void TearDown() { if (Directory.Exists(dir)) Directory.Delete(dir, true); }

        static GuildState Deliver(GuildState g, string id)
        {
            Assert.IsTrue(g.Accept(id, out _));
            Assert.IsTrue(g.PickUp(g.Find(id).ObjectiveItem, out _));
            Assert.IsTrue(g.Deliver(out _));
            return g;
        }

        [Test]
        public void Delivery_PaysServiceAndTrustOnce_ReplayPaysNothing()
        {
            var g = Deliver(GuildState.Prototype(), "c1");
            Assert.AreEqual(30, g.Wallet);
            Assert.IsTrue(g.Services.Contains("c1"));
            Assert.AreEqual(1, g.TrustPoints);
            Deliver(g, "c1");
            Assert.AreEqual(30, g.Wallet, "replay pays nothing");
            Assert.AreEqual(1, g.TrustPoints);
        }

        [Test]
        public void SecondPickUp_IsImpossible()
        {
            var g = GuildState.Prototype();
            g.Accept("c1", out _);
            Assert.IsTrue(g.PickUp("Tool crate", out _));
            Assert.IsFalse(g.PickUp("Tool crate", out var reason));
            Assert.IsNotNull(reason);
        }

        [Test]
        public void DuplicateDeliveryAttempt_IsRefused()
        {
            var g = Deliver(GuildState.Prototype(), "c2");
            Assert.IsFalse(g.Deliver(out _));
            Assert.AreEqual(30, g.Wallet);
        }

        [Test]
        public void Abandon_KeepsRewards_LosesOnlyObjective_ZeroWalletRetry()
        {
            var g = Deliver(GuildState.Prototype(), "c1");
            g.Accept("c2", out _);
            g.PickUp("Supplies", out _);
            g.ResetAttempt();
            Assert.IsNull(g.Carried);
            Assert.AreEqual(AttemptState.Available, g.Attempts["c2"]);
            Assert.AreEqual(30, g.Wallet);
            Assert.IsTrue(g.Services.Contains("c1"));
            g.Wallet = 0;
            Assert.IsTrue(g.Accept("c2", out _), "no coin is needed to retry");
        }

        [Test]
        public void Attunement_OfferedAfterFirstDelivery_SurvivesQuitImmediately()
        {
            var g = GuildState.Prototype();
            Assert.IsFalse(g.CompleteAttunement(out _));
            Deliver(g, "c1");
            Assert.IsTrue(g.CompleteAttunement(out _));
            var store = new SaveStore(dir);
            store.Write(g.ToSnapshot(0));   // saved at attunement completion; the player quits now
            var loaded = GuildState.Prototype();
            loaded.Apply(store.Read());
            Assert.IsTrue(loaded.Attuned);
        }

        [Test]
        public void ReloadMidAttempt_ResetsAttempt_NoPendingState()
        {
            var g = Deliver(GuildState.Prototype(), "c1");
            g.Accept("c2", out _);
            g.PickUp("Supplies", out _);
            var store = new SaveStore(dir);
            store.Write(g.ToSnapshot(1));
            var loaded = GuildState.Prototype();
            loaded.Apply(store.Read());
            Assert.IsNull(loaded.Active);
            Assert.IsNull(loaded.Carried);
            Assert.AreEqual(AttemptState.Available, loaded.Attempts["c2"]);
            Assert.AreEqual(AttemptState.Delivered, loaded.Attempts["c1"]);
            Assert.AreEqual(30, loaded.Wallet);
        }

        [Test]
        public void InterruptedSaveWrite_LoadsPreviousFile()
        {
            var store = new SaveStore(dir);
            var g = Deliver(GuildState.Prototype(), "c1");
            store.Write(g.ToSnapshot(0));
            Deliver(g, "c2");
            store.Write(g.ToSnapshot(0));
            File.WriteAllText(store.Current, "{\"wal");   // the next write was cut off mid-file
            var s = store.Read();
            Assert.IsNotNull(s);
            Assert.AreEqual(30, s.wallet, "previous complete snapshot (after the first delivery)");
        }

        [Test]
        public void EmptySaveFile_IsTreatedAsMissing()
        {
            var store = new SaveStore(dir);
            File.WriteAllText(store.Current, "{}");
            Assert.IsNull(store.Read());
        }
    }
}
