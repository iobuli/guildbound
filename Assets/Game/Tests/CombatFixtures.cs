using System;
using System.Collections.Generic;
using System.Linq;
using Guildbound.Combat;
using Guildbound.Contracts;
using NUnit.Framework;
using UnityEngine;

namespace Guildbound.Tests
{
    // M0.5 named test matrix (GDD v5.1 section 11). Each scenario has a stated expected result and runs at 30, 60
    // and 120 fps; the resolved event log must be identical at every rate.
    public class CombatFixtures
    {
        static readonly float[] Rates = { 30f, 60f, 120f };

        class Scenario
        {
            public Action<CombatWorld> Setup;
            public Func<double, double, Intent[]> Script;   // (frame start time, frame length) -> intents per fighter
            public float Seconds = 2f;
        }

        static bool At(double now, double frame, double t) => now <= t + 1e-9 && now + frame > t + 1e-9;

        static (CombatWorld world, List<ContactEvent> contacts, List<string> log) Run(Scenario s, float fps)
        {
            var world = new CombatWorld();
            s.Setup(world);
            var runner = new FixedStepRunner(world);
            var contacts = new List<ContactEvent>();
            var log = new List<string>();
            double frame = 1.0 / fps;
            int frames = (int)Math.Round(s.Seconds * fps);
            for (int k = 0; k < frames; k++)
            {
                double now = k * frame;
                runner.Frame((float)frame, s.Script(now, frame));
                foreach (var c in world.Contacts)
                {
                    contacts.Add(c);
                    log.Add($"{c.Time:F4} {c.AttackerId}->{c.DefenderId} {c.Action} {c.Outcome}");
                }
                foreach (var st in world.Started) log.Add($"start {st.fighter} {st.state.Kind}");
                world.ClearEvents();
            }
            log.Add($"end hp={string.Join(",", world.Fighters.Select(f => f.Hp.ToString("F2")))} guard={string.Join(",", world.Fighters.Select(f => f.Guard.ToString("F2")))}");
            return (world, contacts, log);
        }

        // Runs at every rate, asserts the logs match, returns the 60 fps result for scenario checks.
        static (CombatWorld world, List<ContactEvent> contacts) RunAll(Scenario s)
        {
            List<string> reference = null;
            (CombatWorld, List<ContactEvent>) at60 = default;
            foreach (var fps in Rates)
            {
                var (w, c, log) = Run(s, fps);
                if (reference == null) reference = log;
                else CollectionAssert.AreEqual(reference, log, $"event log differs at {fps} fps");
                if (fps == 60f) at60 = (w, c);
            }
            return at60;
        }

        static Vector3 V(float x, float z) => new Vector3(x, 0, z);
        static Intent[] I(params Intent[] i) => i;

        static Scenario Duel(float distance, Func<double, double, Intent[]> script, float seconds = 2f, bool defenderFacesAway = false, float defenderHp = 60f)
            => new Scenario
            {
                Seconds = seconds,
                Script = script,
                Setup = w =>
                {
                    w.Add(V(0, 0), Vector3.forward, 100f);
                    w.Add(V(0, distance), defenderFacesAway ? Vector3.forward : Vector3.back, defenderHp);
                },
            };

        [Test]
        public void LightOutOfRange_NoContact()
        {
            var r = RunAll(Duel(2.0f, (t, f) => I(new Intent { Light = At(t, f, 0) }, default)));
            Assert.AreEqual(0, r.contacts.Count);
            Assert.AreEqual(60f, r.world.Fighters[1].Hp);
        }

        [Test]
        public void FrontGuard_BlocksLight_AndDrains()
        {
            var r = RunAll(Duel(1.0f, (t, f) => I(new Intent { Light = At(t, f, 0.1) }, new Intent { GuardHeld = true })));
            Assert.AreEqual(1, r.contacts.Count);
            Assert.AreEqual(Outcome.Blocked, r.contacts[0].Outcome);
            Assert.AreEqual(60f, r.world.Fighters[1].Hp);
            Assert.AreEqual(85f, r.world.Fighters[1].Guard, 0.01f);
        }

        [Test]
        public void GuardFacingAway_IsHitFromBehind_OncePerAction()
        {
            var r = RunAll(Duel(1.0f, (t, f) => I(new Intent { Heavy = At(t, f, 0) }, new Intent { GuardHeld = true }), defenderFacesAway: true));
            Assert.AreEqual(1, r.contacts.Count, "one outcome per target per action");
            Assert.AreEqual(Outcome.Hit, r.contacts[0].Outcome);
            Assert.AreEqual(60f - 12f, r.world.Fighters[1].Hp, 0.01f);
        }

        [Test]
        public void GuardCapacityEmpties_ThenBreaks()
        {
            var r = RunAll(Duel(1.0f, (t, f) => I(new Intent { Heavy = At(t, f, 0) || At(t, f, 1.2) || At(t, f, 2.4) }, new Intent { GuardHeld = true }), seconds: 3.6f));
            var outcomes = r.contacts.Select(c => c.Outcome).ToArray();
            CollectionAssert.AreEqual(new[] { Outcome.Blocked, Outcome.Blocked, Outcome.GuardBroken }, outcomes);
            Assert.AreEqual(60f, r.world.Fighters[1].Hp, "guard break deals no damage");
        }

        [Test]
        public void LightPhrase_TwoActions_TwoContactIdentities()
        {
            var r = RunAll(Duel(1.0f, (t, f) => I(new Intent { Light = At(t, f, 0) || At(t, f, 0.5) }, default)));
            Assert.AreEqual(2, r.contacts.Count);
            Assert.AreEqual(ActionKind.LightL, r.contacts[0].Action);
            Assert.AreEqual(ActionKind.LightR, r.contacts[1].Action);
            Assert.AreNotEqual(r.contacts[0].ActionSequence, r.contacts[1].ActionSequence);
            Assert.AreEqual(60f - 12f, r.world.Fighters[1].Hp, 0.01f);
        }

        [Test]
        public void Throw_BeatsGuard()
        {
            var r = RunAll(Duel(1.0f, (t, f) => I(new Intent { Throw = At(t, f, 0) }, new Intent { GuardHeld = true })));
            Assert.AreEqual(1, r.contacts.Count);
            Assert.AreEqual(ActionKind.Throw, r.contacts[0].Action);
            Assert.AreEqual(Outcome.Hit, r.contacts[0].Outcome);
            Assert.AreEqual(60f - 8f, r.world.Fighters[1].Hp, 0.01f);
        }

        [Test]
        public void StrikeDuringReachIn_BeatsThrow()
        {
            var r = RunAll(Duel(1.0f, (t, f) => I(new Intent { Throw = At(t, f, 0) }, new Intent { Light = At(t, f, 0) })));
            Assert.IsTrue(r.contacts.All(c => c.Action != ActionKind.Throw || c.Outcome != Outcome.Hit), "throw must not land");
            Assert.AreEqual(1, r.contacts.Count(c => c.AttackerId == 1 && c.Outcome == Outcome.Hit));
            Assert.AreEqual(100f - 6f, r.world.Fighters[0].Hp, 0.01f);
        }

        [Test]
        public void Step_EvadesHeavy()
        {
            var r = RunAll(Duel(1.0f, (t, f) => I(new Intent { Heavy = At(t, f, 0) }, new Intent { Step = At(t, f, 0.4), Move = At(t, f, 0.4) ? new Vector2(1, 0) : Vector2.zero })));
            Assert.AreEqual(0, r.contacts.Count);
            Assert.AreEqual(60f, r.world.Fighters[1].Hp);
        }

        [Test]
        public void ChargeInterrupted_SpendsCooldown_NoProjectile()
        {
            var s = Duel(1.0f, (t, f) => I(new Intent { Cast = At(t, f, 0), CastHeld = true }, new Intent { Light = At(t, f, 0.5) }));
            var setup = s.Setup;
            s.Setup = w => { setup(w); w.Fighters[0].Attuned = true; };
            var r = RunAll(s);
            Assert.IsTrue(r.contacts.Any(c => c.AttackerId == 1 && c.Outcome == Outcome.Hit), "light interrupts the hold");
            Assert.IsFalse(r.contacts.Any(c => c.AttackerId == 0), "no projectile after interruption");
            Assert.Greater(r.world.Fighters[0].CastCooldown, 0f, "interruption after commitment spends the cooldown");
        }

        [Test]
        public void CastMovedBeforeCommit_Cancels_KeepsCooldown()
        {
            var r = RunAll(Duel(5.0f, (t, f) => I(new Intent { Cast = At(t, f, 0), Move = t >= 0.2 && t < 0.3 ? new Vector2(1, 0) : Vector2.zero }, default)));
            Assert.AreEqual(0, r.contacts.Count);
            Assert.AreEqual(0f, r.world.Fighters[0].CastCooldown);
            Assert.AreEqual(0, r.world.Projectiles.Count);
        }

        [Test]
        public void PlainCast_HitsAtRange()
        {
            var r = RunAll(Duel(5.0f, (t, f) => I(new Intent { Cast = At(t, f, 0) }, default)));
            Assert.AreEqual(1, r.contacts.Count);
            Assert.AreEqual(ActionKind.CastPlain, r.contacts[0].Action);
            Assert.AreEqual(Outcome.Hit, r.contacts[0].Outcome);
            Assert.AreEqual(60f - 10f, r.world.Fighters[1].Hp, 0.01f);
        }

        [Test]
        public void ChargedCast_GuardTakesNoDamage_IsPushedAndDrained()
        {
            var s = Duel(5.0f, (t, f) => I(new Intent { Cast = At(t, f, 0), CastHeld = t < 1.2 }, new Intent { GuardHeld = true }), seconds: 2.5f);
            var setup = s.Setup;
            s.Setup = w => { setup(w); w.Fighters[0].Attuned = true; };
            var r = RunAll(s);
            Assert.AreEqual(1, r.contacts.Count);
            Assert.AreEqual(ActionKind.CastRelease, r.contacts[0].Action);
            Assert.AreEqual(Outcome.Blocked, r.contacts[0].Outcome);
            Assert.AreEqual(60f, r.world.Fighters[1].Hp);
            Assert.Greater(r.world.Fighters[1].Position.z, 5.5f, "pushed back");
        }

        [Test]
        public void MutualThrows_BothStagger()
        {
            var r = RunAll(Duel(1.0f, (t, f) => I(new Intent { Throw = At(t, f, 0) }, new Intent { Throw = At(t, f, 0) })));
            Assert.AreEqual(2, r.contacts.Count);
            Assert.IsTrue(r.contacts.All(c => c.Outcome == Outcome.Interrupted));
            Assert.AreEqual(100f, r.world.Fighters[0].Hp);
            Assert.AreEqual(60f, r.world.Fighters[1].Hp);
        }

        [Test]
        public void ThrowAtWall_IsAVisibleFailure()
        {
            var s = new Scenario
            {
                Seconds = 1.5f,
                Script = (t, f) => I(new Intent { Throw = At(t, f, 0) }, default),
                Setup = w => { w.Add(V(0, 5.0f), Vector3.forward, 100f); w.Add(V(0, 5.6f), Vector3.back, 60f); },
            };
            var r = RunAll(s);
            Assert.AreEqual(1, r.contacts.Count);
            Assert.AreEqual(Outcome.Miss, r.contacts[0].Outcome);
            Assert.AreEqual(60f, r.world.Fighters[1].Hp);
            Assert.LessOrEqual(r.world.Fighters[1].Position.z, 6f - 0.3f + 1e-4f, "nobody through the wall");
        }

        [Test]
        public void Recover_OutOfCombat_RestoresFullHp_ConsumesItemOnlyOnCompletion()
        {
            var s = new Scenario
            {
                Seconds = 1.6f,
                Script = (t, f) => I(new Intent { Recover = At(t, f, 0) }, default),
                Setup = w => { var a = w.Add(V(5.5f, 5.5f), Vector3.forward, 100f); a.Hp = 40f; a.PrepItems = 2; w.Add(V(-5.5f, -5.5f), Vector3.forward, 60f); },
            };
            var r = RunAll(s);
            Assert.AreEqual(100f, r.world.Fighters[0].Hp);
            Assert.AreEqual(1, r.world.Fighters[0].PrepItems);
        }

        [Test]
        public void Recover_RefusedWithThreatNear()
        {
            var r = RunAll(new Scenario
            {
                Seconds = 3.5f,
                Script = (t, f) => I(new Intent { Recover = At(t, f, 0) }, default),
                Setup = w => { var a = w.Add(V(0, 0), Vector3.forward, 100f); a.Hp = 40f; w.Add(V(0, 4f), Vector3.back, 60f); },
            });
            Assert.AreEqual(40f, r.world.Fighters[0].Hp);
        }

        [Test]
        public void Recover_InterruptedByMove_NoHeal_ItemKept()
        {
            var r = RunAll(new Scenario
            {
                Seconds = 3.5f,
                Script = (t, f) => I(new Intent { Recover = At(t, f, 0), Move = t >= 0.5 && t < 0.6 ? new Vector2(1, 0) : Vector2.zero }, default),
                Setup = w => { var a = w.Add(V(5.5f, 5.5f), Vector3.forward, 100f); a.Hp = 40f; a.PrepItems = 1; w.Add(V(-5.5f, -5.5f), Vector3.forward, 60f); },
            });
            Assert.AreEqual(40f, r.world.Fighters[0].Hp);
            Assert.AreEqual(1, r.world.Fighters[0].PrepItems);
        }
    }
}
