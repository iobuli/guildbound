using System.IO;
using Guildbound.Combat;
using Guildbound.Contracts;
using Guildbound.Progress;
using NUnit.Framework;
using UnityEngine;

namespace Guildbound.Tests
{
    // Gold path steps 2-9 (GDD v5.1 section 3) driven on the real rules: GuildState for obligations and the
    // CombatWorld for each fight. Scene steps (1 wake, door, routes) are verified later in Play mode.
    public class GoldPathFixture
    {
        // Player fights a 60 HP opponent with light phrases until it is down. Returns hits counted with contact.
        static int Fight(bool playerAttuned, out bool playerWon, out int hitsWithoutContact)
        {
            var w = new CombatWorld();
            var p = w.Add(new Vector3(0, 0, 0), Vector3.forward, 100f);
            var o = w.Add(new Vector3(0, 0, 1.0f), Vector3.back, 60f);
            p.Attuned = playerAttuned;
            p.LockTarget = o.Id;
            var runner = new FixedStepRunner(w);
            int hits = 0;
            hitsWithoutContact = 0;
            for (int frame = 0; frame < 60 * 30 && !o.Defeated && !p.Defeated; frame++)
            {
                var to = o.Position - p.Position;   // close the distance knockback opens, strike only in reach
                bool inReach = to.magnitude <= 1.0f;
                bool press = inReach && frame % 39 == 0;
                var move = inReach ? Vector2.zero : new Vector2(to.x, to.z).normalized;
                runner.Frame(1f / 60f, new[] { new Intent { Light = press, Move = move }, default(Intent) });
                foreach (var c in w.Contacts)
                {
                    if (c.Outcome != Outcome.Hit || c.AttackerId != p.Id) continue;
                    hits++;
                    // every damaging contact must sit within fist reach of the defender at that tick
                    if (Vector3.Distance(c.Point, w.Fighters[c.DefenderId].Position) > w.T.FistRadius + w.T.BodyRadius + 0.2f) hitsWithoutContact++;
                }
                w.ClearEvents();
            }
            playerWon = o.Defeated && !p.Defeated;
            return hits;
        }

        [Test]
        public void GoldPath_Steps2To9()
        {
            var dir = Path.Combine(Path.GetTempPath(), "guildbound-gold-" + System.Guid.NewGuid().ToString("N"));
            try
            {
                var store = new SaveStore(dir);
                var g = GuildState.Prototype();

                // 2. accept obligation A; route, payment and service visible before accepting
                var a = g.Find("c1");
                Assert.IsNotEmpty(a.Route); Assert.Greater(a.Payment, 0); Assert.IsNotEmpty(a.Service);
                Assert.IsTrue(g.Accept("c1", out _));
                store.Write(g.ToSnapshot(0));

                // 4. first opponent defeated with zero hits registered without contact
                int hits = Fight(false, out bool won, out int ghost);
                Assert.IsTrue(won, "player defeats the first opponent");
                Assert.Greater(hits, 0);
                Assert.AreEqual(0, ghost, "zero hits without contact");

                // 5. recover the objective; a second pick-up is impossible
                Assert.IsTrue(g.PickUp(a.ObjectiveItem, out _));
                Assert.IsFalse(g.PickUp(a.ObjectiveItem, out _));

                // 6. deliver: payment + service + new option in one event
                Assert.IsTrue(g.Deliver(out _));
                Assert.AreEqual(a.Payment, g.Wallet);
                Assert.IsTrue(g.Services.Contains("c1"));
                Assert.IsTrue(g.AttunementOffered, "new option: attunement offered after the first delivery");
                store.Write(g.ToSnapshot(0));

                // 7. attunement exercise: charged cast unlocked and performed once
                Assert.IsTrue(g.CompleteAttunement(out _));
                store.Write(g.ToSnapshot(0));
                var w = new CombatWorld();
                var p = w.Add(Vector3.zero, Vector3.forward, 100f);
                w.Add(new Vector3(0, 0, 5f), Vector3.back, 60f);
                p.Attuned = g.Attuned;
                var r = new FixedStepRunner(w);
                bool charged = false;
                for (int f = 0; f < 60 * 3; f++)
                {
                    r.Frame(1f / 60f, new[] { new Intent { Cast = f == 0, CastHeld = f < 60 }, default(Intent) });
                    foreach (var c in w.Contacts) if (c.Action == ActionKind.CastRelease) charged = true;
                    w.ClearEvents();
                }
                Assert.IsTrue(charged, "charged cast performed once");

                // 8. second obligation recovered and delivered exactly once
                Assert.IsTrue(g.Accept("c2", out _));
                Fight(true, out bool won2, out _);
                Assert.IsTrue(won2);
                Assert.IsTrue(g.PickUp(g.Find("c2").ObjectiveItem, out _));
                Assert.IsTrue(g.Deliver(out _));
                Assert.IsFalse(g.Deliver(out _), "exactly once");
                Assert.AreEqual(60, g.Wallet);
                Assert.IsTrue(g.Services.Contains("c2"));
                store.Write(g.ToSnapshot(0));

                // 9. free practice replay: defeat once, retry from the bench with rewards and services intact, zero coin
                Assert.IsTrue(g.Accept("c1", out _));
                g.ResetAttempt();   // defeat
                var reloaded = GuildState.Prototype();
                reloaded.Apply(store.Read());
                Assert.AreEqual(60, reloaded.Wallet);
                Assert.IsTrue(reloaded.Services.Contains("c1") && reloaded.Services.Contains("c2"));
                Assert.IsTrue(reloaded.Attuned);
                reloaded.Wallet = 0;
                Assert.IsTrue(reloaded.Accept("c1", out _), "objective available again, zero coin needed");
            }
            finally
            {
                if (Directory.Exists(dir)) Directory.Delete(dir, true);
            }
        }
    }
}
