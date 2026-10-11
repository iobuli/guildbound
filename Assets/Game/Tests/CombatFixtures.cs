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
            public IWorldGeometry Geometry;   // null = the sparring BoxArena
        }

        // A thin wall across the world at z = Z (infinite in x), for world-first scenarios without Unity physics.
        sealed class WallAtZ : IWorldGeometry
        {
            readonly float z;
            public WallAtZ(float z) => this.z = z;

            public Vector3 Constrain(Vector3 from, Vector3 to, float r)
                => new Vector3(to.x, 0, from.z < z ? Mathf.Min(to.z, z - r) : Mathf.Max(to.z, z + r));

            public bool Blocked(Vector3 from, Vector3 to, float r, out float t)
            {
                t = 2;
                float plane = from.z < z ? z - r : z + r;
                if (from.z < z ? to.z <= plane : to.z >= plane) return false;
                t = Mathf.Clamp01((plane - from.z) / (to.z - from.z));
                return true;
            }

            public bool Fits(Vector3 at, float r) => Mathf.Abs(at.z - z) >= r;
        }

        static bool At(double now, double frame, double t) => now <= t + 1e-9 && now + frame > t + 1e-9;

        static (CombatWorld world, List<ContactEvent> contacts, List<string> log) Run(Scenario s, float fps)
        {
            var world = new CombatWorld(null, s.Geometry);
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

        // ---------------- world first (M1 W1): walls stop bodies, strikes, throws and fire ----------------

        static Scenario Walled(Scenario s, float wallZ) { s.Geometry = new WallAtZ(wallZ); return s; }

        // Open ground with no walls at all, for route-scale scenarios.
        sealed class Open : IWorldGeometry
        {
            public Vector3 Constrain(Vector3 from, Vector3 to, float r) => new Vector3(to.x, 0, to.z);
            public bool Blocked(Vector3 from, Vector3 to, float r, out float t) { t = 2; return false; }
            public bool Fits(Vector3 at, float r) => true;
        }

        [Test]
        public void Walk_IntoAWall_StopsAtTheWall()
        {
            var r = RunAll(Walled(new Scenario
            {
                Seconds = 1.5f,
                Script = (t, f) => I(new Intent { Move = new Vector2(0, 1) }, default),
                Setup = w => { w.Add(V(0, 0), Vector3.forward, 100f); w.Add(V(5f, -5f), Vector3.forward, 60f); },
            }, 1.5f));
            Assert.AreEqual(1.5f - 0.3f, r.world.Fighters[0].Position.z, 1e-4f, "stops one body radius short of the wall");
        }

        [Test]
        public void Light_ThroughAWall_NoContact()
        {
            Func<double, double, Intent[]> light = (t, f) => I(new Intent { Light = At(t, f, 0) }, default);
            var open = RunAll(Duel(1.0f, light));
            Assert.AreEqual(Outcome.Hit, open.contacts.Single().Outcome, "control: the same strike lands with no wall");

            var r = RunAll(Walled(Duel(1.0f, light), 0.5f));
            Assert.AreEqual(0, r.contacts.Count);
            Assert.AreEqual(60f, r.world.Fighters[1].Hp);
        }

        [Test]
        public void Throw_AtATargetBehindAWall_IsAVisibleFailure()
        {
            Func<double, double, Intent[]> grab = (t, f) => I(new Intent { Throw = At(t, f, 0) }, default);
            var open = RunAll(Duel(0.9f, grab, seconds: 1.5f));
            Assert.AreEqual(Outcome.Hit, open.contacts.Single().Outcome, "control: the same throw lands with no wall");

            var r = RunAll(Walled(Duel(0.9f, grab, seconds: 1.5f), 0.45f));
            Assert.AreEqual(Outcome.Miss, r.contacts.Single().Outcome);
            Assert.AreEqual(60f, r.world.Fighters[1].Hp);
            Assert.GreaterOrEqual(r.world.Fighters[1].Position.z, 0.45f + 0.3f - 1e-4f, "the target stays on its side");
        }

        [Test]
        public void Throw_WithTheLandingBehindAWall_IsAVisibleFailure()
        {
            var r = RunAll(Walled(Duel(0.8f, (t, f) => I(new Intent { Throw = At(t, f, 0) }, default), seconds: 1.5f), 1.25f));
            Assert.AreEqual(Outcome.Miss, r.contacts.Single().Outcome);
            Assert.AreEqual(60f, r.world.Fighters[1].Hp);
            Assert.LessOrEqual(r.world.Fighters[1].Position.z, 1.25f - 0.3f + 1e-4f, "nobody through the wall");
        }

        [Test]
        public void PlainCast_StopsAtTheWall_BeforeAFighterBehindIt()
        {
            var r = RunAll(Walled(Duel(5.0f, (t, f) => I(new Intent { Cast = At(t, f, 0) }, default)), 2.5f));
            var c = r.contacts.Single();
            Assert.AreEqual(Outcome.Miss, c.Outcome);
            Assert.AreEqual(2.5f, c.Point.z, 1e-3f, "the projectile ends at the wall");
            Assert.AreEqual(60f, r.world.Fighters[1].Hp);
        }

        [Test]
        public void PointBlankCast_AtAWall_NeverStartsBehindIt()
        {
            var r = RunAll(Walled(Duel(1.5f, (t, f) => I(new Intent { Cast = At(t, f, 0) }, default)), 0.4f));
            var c = r.contacts.Single();
            Assert.AreEqual(Outcome.Miss, c.Outcome);
            Assert.AreEqual(0.4f, c.Point.z, 1e-3f);
            Assert.AreEqual(60f, r.world.Fighters[1].Hp);
        }

        // Defender 0.35 m behind a thin wall: the projectile's 0.25 m radius reaches the body one tick before the
        // projectile reaches the wall (spawn 0.6 m, 0.117 m per tick, endpoint 1.417 m is 0.43 m from the defender).
        [Test]
        public void PlainCast_AtAFighterJustBehindAWall_StopsAtTheWall()
        {
            var r = RunAll(Walled(Duel(1.85f, (t, f) => I(new Intent { Cast = At(t, f, 0) }, default)), 1.5f));
            var c = r.contacts.Single();
            Assert.AreEqual(Outcome.Miss, c.Outcome);
            Assert.AreEqual(1.5f, c.Point.z, 1e-3f);
            Assert.AreEqual(60f, r.world.Fighters[1].Hp);
        }

        // Same placement for the charged blast; the wall is 8 ticks from the release, so the release and the impact never
        // share a 30 fps frame (the harness logs a frame's contacts before its starts).
        [Test]
        public void ChargedBlast_DoesNotReachThroughAWall()
        {
            var s = Walled(Duel(1.85f, (t, f) => I(new Intent { Cast = At(t, f, 0), CastHeld = t < 1.2 }, default), seconds: 2.5f), 1.5f);
            var setup = s.Setup;
            s.Setup = w => { setup(w); w.Fighters[0].Attuned = true; };
            var r = RunAll(s);
            var c = r.contacts.Single();
            Assert.AreEqual(ActionKind.CastRelease, c.Action);
            Assert.AreEqual(Outcome.Miss, c.Outcome, "the defender is inside the radius but behind the wall");
            Assert.AreEqual(60f, r.world.Fighters[1].Hp);
            Assert.AreEqual(1.85f, r.world.Fighters[1].Position.z, 1e-4f, "and is not pushed");
        }

        // ---------------- motion slice: bounded turning, run carry ----------------

        [Test]
        public void Unlocked_ReversalTurnsAtTheBoundedRate_LockOnStillFacesTheTarget()
        {
            var w = new CombatWorld();
            var f = w.Add(V(0, 0), Vector3.forward, 100f);
            var o = w.Add(V(3, 3), Vector3.forward, 60f);
            var back = new[] { new Intent { Move = new Vector2(0, -1) }, default };
            int quarter = Mathf.RoundToInt(90f / (w.T.TurnRate * Tuning.Step));   // 15 ticks at 720 deg/s
            for (int k = 0; k < quarter; k++) w.Tick(back);
            Assert.AreEqual(90f, Vector3.Angle(Vector3.forward, f.Facing), 0.5f, "half way through the reversal");
            Assert.AreEqual(0f, f.Facing.y, "the turn stays in the ground plane");
            for (int k = 0; k < quarter; k++) w.Tick(back);
            Assert.AreEqual(180f, Vector3.Angle(Vector3.forward, f.Facing), 0.5f);
            Assert.Less(f.Position.z, 0f, "the body moved back from the first tick; only the facing turns");

            f.LockTarget = o.Id;
            var side = new[] { new Intent { Move = new Vector2(1, 0) }, default };
            for (int k = 0; k < 30; k++)
            {
                w.Tick(side);
                Assert.AreEqual(0f, Vector3.Angle((o.Position - f.Position).normalized, f.Facing), 0.05f, "locked: faces the target every tick");
            }
        }

        // Run 12 ticks (0.4 m) then press light at 0.1 s: the carry slides 0.3 m more, so the fist reaches a defender at
        // 1.7 m. Control: run until 0.06 s (8 ticks at 30/60/120 fps) and stand 4 ticks before the press (no carry): the same light falls short.
        static Scenario RunThenLight(float runUntil, float defenderAt, float seconds = 1.2f) => Duel(defenderAt,
            (t, f) => I(new Intent { Move = t < runUntil ? new Vector2(0, 1) : Vector2.zero, Run = t < runUntil, Light = At(t, f, 0.1) }, default),
            seconds);

        [Test]
        public void RunningStrike_CarriesTheRun_StandingStrikeDoesNot()
        {
            var control = RunAll(RunThenLight(0.06f, 1.7f));
            Assert.AreEqual(0, control.contacts.Count, "control: no carry, out of reach");

            var r = RunAll(RunThenLight(0.1f, 1.7f));
            Assert.AreEqual(Outcome.Hit, r.contacts.Single().Outcome);
            Assert.AreEqual(0.4f + 0.3f, r.world.Fighters[0].Position.z, 0.02f, "run 0.4 m + carry 0.3 m, then planted");
        }

        [Test]
        public void RunCarry_StopsAtAWall()
        {
            var r = RunAll(Walled(RunThenLight(0.1f, 1.7f), 0.85f));
            Assert.AreEqual(0.85f - 0.3f, r.world.Fighters[0].Position.z, 1e-4f, "the slide ends one body radius short of the wall");
            Assert.AreEqual(60f, r.world.Fighters[1].Hp);
        }

        // ---------------- motion slice C/D (Aura rulings): cancel on hit only, bounded wind-up retarget ----------------

        // The second press lands at 11/30 s: the same tick at 30/60/120 fps, past the active window (0.35 s) and 0.28 s
        // before the end of recovery, outside the 0.25 s buffer. Without a cancel that press is simply dropped.
        static Scenario PressTwice(float distance, Intent defender = default)
            => Duel(distance, (t, f) => I(new Intent { Light = At(t, f, 0) || At(t, f, 11.0 / 30) }, defender), seconds: 1.2f);

        static int Starts(Scenario s, string kind) => Run(s, 60f).log.Count(l => l.StartsWith("start 0 " + kind));

        [Test]
        public void Hit_CancelsRecovery_IntoTheNextStrike()
        {
            var r = RunAll(PressTwice(1.0f));
            CollectionAssert.AreEqual(new[] { Outcome.Hit, Outcome.Hit }, r.contacts.Select(c => c.Outcome).ToArray());
            CollectionAssert.AreEqual(new[] { ActionKind.LightL, ActionKind.LightR }, r.contacts.Select(c => c.Action).ToArray(), "the phrase continues");
            Assert.Less(r.contacts[1].Time, 0.65 + 0.25, "the second strike started on the press, not after the full recovery");
        }

        [Test]
        public void Whiff_KeepsFullRecovery_NoCancel()
        {
            var r = RunAll(PressTwice(2.0f));
            Assert.AreEqual(0, r.contacts.Count);
            Assert.AreEqual(1, Starts(PressTwice(2.0f), "Light"), "the second press is dropped, as before");
        }

        [Test]
        public void Blocked_KeepsFullRecovery_NoCancel()
        {
            var r = RunAll(PressTwice(1.0f, new Intent { GuardHeld = true }));
            Assert.AreEqual(Outcome.Blocked, r.contacts.Single().Outcome);
            Assert.AreEqual(1, Starts(PressTwice(1.0f, new Intent { GuardHeld = true }), "Light"), "a blocked strike stays punishable");
        }

        [Test]
        public void Retarget_AtMostFifteenDegrees_OnlyDuringStartup_NeverWhenLocked()
        {
            var right = new Vector2(1, 0);

            var w = new CombatWorld();
            var f = w.Add(V(0, 0), Vector3.forward, 100f);
            w.Add(V(4, 4), Vector3.forward, 60f);
            w.Tick(new[] { new Intent { Light = true, Move = right }, default });
            for (int k = 1; k < 40; k++) w.Tick(new[] { new Intent { Move = right }, default });   // into the active window
            Assert.AreEqual(ActionKind.LightL, f.State.Kind);
            Assert.AreEqual(w.T.RetargetMax, Vector3.Angle(Vector3.forward, f.Facing), 0.01f, "held input turns 15 deg, no more");

            var w2 = new CombatWorld();
            var g = w2.Add(V(0, 0), Vector3.forward, 100f);
            w2.Add(V(4, 4), Vector3.forward, 60f);
            w2.Tick(new[] { new Intent { Light = true }, default });
            for (int k = 1; k < 32; k++) w2.Tick(null);                                              // past startup
            for (int k = 0; k < 10; k++) w2.Tick(new[] { new Intent { Move = right }, default });
            Assert.AreEqual(0f, Vector3.Angle(Vector3.forward, g.Facing), 0.01f, "no turn once Active has begun");

            var w3 = new CombatWorld();
            var h = w3.Add(V(0, 0), Vector3.forward, 100f);
            var o = w3.Add(V(4, 4), Vector3.forward, 60f);
            h.LockTarget = o.Id;
            w3.Tick(new[] { new Intent { Light = true, Move = right }, default });
            for (int k = 1; k < 40; k++) w3.Tick(new[] { new Intent { Move = right }, default });
            Assert.AreEqual(0f, Vector3.Angle(Vector3.forward, h.Facing), 0.01f, "locked strikes keep their facing");
        }

        // Defender 20 deg off the attacker's facing at 1.15 m: a straight light misses by 0.08 m; turned 15 deg in the
        // wind-up, the SAME sweep connects. Proves the rules facing and the contact sweep are one source.
        static Scenario Offset20(bool steer)
        {
            var dir = new Vector2(Mathf.Sin(20f * Mathf.Deg2Rad), Mathf.Cos(20f * Mathf.Deg2Rad));
            return new Scenario
            {
                Seconds = 1.0f,
                Script = (t, f) => I(new Intent { Light = At(t, f, 0), Move = steer ? dir : Vector2.zero }, default),
                Setup = w => { w.Add(V(0, 0), Vector3.forward, 100f); w.Add(V(dir.x * 1.15f, dir.y * 1.15f), Vector3.back, 60f); },
            };
        }

        [Test]
        public void Retarget_TurnsTheContactSweep_NotJustTheBody()
        {
            Assert.AreEqual(0, RunAll(Offset20(false)).contacts.Count, "control: the straight strike misses");
            Assert.AreEqual(Outcome.Hit, RunAll(Offset20(true)).contacts.Single().Outcome);
        }

        // ---------------- M1 W3: opponent posts (watch 12 m, leash 20 m, line of sight) ----------------

        // Player (fighter 0) walks +Z along x = passX from z = -15 for 14 s; the posted opponent (fighter 1) at the origin
        // only moves when its post says fight. Returns whether it ever engaged.
        static bool WalkPast(float passX)
        {
            var w = new CombatWorld(null, new Open());
            var player = w.Add(V(passX, -15f), Vector3.forward, 100f);
            var opp = w.Add(V(0, 0), Vector3.back, 60f);
            var post = new OpponentPost(opp.Position);
            bool engaged = false;
            for (int k = 0; k < 120 * 14; k++)
            {
                engaged |= post.Update(w, opp, player, out var home);
                w.Tick(new[] { new Intent { Move = new Vector2(0, 1) }, new Intent { Move = home } });
            }
            return engaged;
        }

        [Test]
        public void Post_PassingOutsideTheWatchRadius_NeverEngages()
        {
            Assert.IsFalse(WalkPast(13f), "closest approach 13 m: the route is passed without a fight");
            Assert.IsTrue(WalkPast(11f), "control: closest approach 11 m engages");
        }

        [Test]
        public void Post_LeashesPast20m_WalksHome_IgnoringTheFoe_ThenWatchesAgain()
        {
            var w = new CombatWorld(null, new Open());
            var player = w.Add(V(10f, 0), Vector3.forward, 100f);
            var opp = w.Add(V(0, 0), Vector3.forward, 60f);
            opp.Hp = 45f;
            var post = new OpponentPost(opp.Position);
            Intent Chase(Vector2 home, bool fight)
            {
                if (!fight) return new Intent { Move = home };
                var to = player.Position - opp.Position;
                return new Intent { Move = new Vector2(to.x, to.z).normalized, Run = true };
            }

            Assert.IsTrue(post.Update(w, opp, player, out _), "10 m with a clear line: engaged");
            int k = 0;
            for (; k < 120 * 20 && !post.Returning; k++)   // the player flees at run speed, the opponent chases
            {
                bool fight = post.Update(w, opp, player, out var home);
                w.Tick(new[] { new Intent { Move = new Vector2(1, 0), Run = true }, Chase(home, fight) });
            }
            Assert.IsTrue(post.Returning, "leashed");
            Assert.Greater(opp.Position.x, 20f - 0.1f);

            for (k = 0; k < 120 * 20; k++)   // the player now trails it home, staying inside 12 m
            {
                bool fight = post.Update(w, opp, player, out var home);
                if (!post.Returning) break;   // home: from here it may watch again
                Assert.IsFalse(fight, "walking home it ignores the foe");
                w.Tick(new[] { new Intent { Move = new Vector2(-1, 0) }, Chase(home, fight) });
                Assert.Less((player.Position - opp.Position).magnitude, 12f);
            }
            Assert.IsFalse(post.Returning, "home again");
            Assert.LessOrEqual(new Vector3(opp.Position.x, 0, opp.Position.z).magnitude, post.HomeTolerance + 0.05f);
            Assert.AreEqual(45f, opp.Hp, "no regen on the way home");
            Assert.IsTrue(post.Update(w, opp, player, out _), "back at its post it watches again");
        }

        [Test]
        public void Post_Watch_NeedsALineOfSight()
        {
            bool Sees(IWorldGeometry g)
            {
                var w = new CombatWorld(null, g);
                var player = w.Add(V(0, 3f), Vector3.back, 100f);
                var opp = w.Add(V(0, -3f), Vector3.forward, 60f);
                return new OpponentPost(opp.Position).Update(w, opp, player, out _);
            }
            Assert.IsTrue(Sees(new Open()), "control: 6 m in the open engages");
            Assert.IsFalse(Sees(new WallAtZ(0f)), "the same 6 m behind a wall does not");
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
