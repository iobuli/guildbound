using System.Collections.Generic;
using Guildbound.Contracts;
using UnityEngine;

namespace Guildbound.Combat
{
    // Deterministic combat rules at a fixed step (GDD v5.1 section 4). Order per tick: intent -> action state ->
    // motor (sole mover) -> swept contact -> one outcome per target per action. No randomness, fighters in index order.
    public class CombatWorld
    {
        public readonly Tuning T;
        public readonly IWorldGeometry Geometry;
        public readonly List<FighterSim> Fighters = new List<FighterSim>();
        public readonly List<Projectile> Projectiles = new List<Projectile>();
        public readonly List<ContactEvent> Contacts = new List<ContactEvent>();
        public readonly List<(int fighter, ActionState state)> Started = new List<(int, ActionState)>();
        public double Time;
        int sequence;
        static readonly Intent Idle = default;

        public CombatWorld(Tuning tuning = null, IWorldGeometry geometry = null)
        {
            T = tuning ?? new Tuning();
            Geometry = geometry ?? new BoxArena(T.ArenaHalfSize);
        }

        public FighterSim Add(Vector3 position, Vector3 facing, float hp)
        {
            var f = new FighterSim(Fighters.Count, position, facing, hp, T.GuardMax);
            Fighters.Add(f);
            return f;
        }

        // Events accumulate until the caller clears them (presentation drains once per rendered frame).
        public void ClearEvents()
        {
            Contacts.Clear();
            Started.Clear();
        }

        public void Tick(IReadOnlyList<Intent> intents)
        {
            float dt = Tuning.Step;
            Time += dt;
            foreach (var f in Fighters) f.moveFrom = f.Position;
            for (int i = 0; i < Fighters.Count; i++)
            {
                var f = Fighters[i];
                if (f.Defeated) { f.Velocity = Vector3.zero; continue; }
                var intent = intents != null && i < intents.Count ? intents[i] : Idle;
                Timers(f, intent, dt);
                ApplyIntent(f, intent);
                Advance(f, intent, dt);
            }
            for (int i = 0; i < Fighters.Count; i++)
                Move(Fighters[i], intents != null && i < intents.Count ? intents[i] : Idle, dt);
            Separate();
            for (int i = 0; i < Fighters.Count; i++) SweepStrike(Fighters[i]);
            AdvanceProjectiles(dt);
        }

        // ---------------- intent and action state ----------------

        void Timers(FighterSim f, Intent intent, float dt)
        {
            if (f.CastCooldown > 0) f.CastCooldown = Mathf.Max(0, f.CastCooldown - dt);
            if (f.stepCooldown > 0) f.stepCooldown = Mathf.Max(0, f.stepCooldown - dt);
            if (f.State.Kind != ActionKind.Guard)
            {
                if (f.guardRegenWait > 0) f.guardRegenWait = Mathf.Max(0, f.guardRegenWait - dt);
                else if (f.State.Kind != ActionKind.GuardBreak) f.Guard = Mathf.Min(T.GuardMax, f.Guard + T.GuardRegenRate * dt);
            }
        }

        void ApplyIntent(FighterSim f, Intent intent)
        {
            if (f.State.Kind == ActionKind.Recover)
            {
                if (intent.AnyPress || intent.Move.sqrMagnitude > 0.04f || intent.GuardHeld) Cancel(f);
                else return;
            }
            if (!f.Free)
            {
                Retarget(f, intent);
                // a strike that HIT may cancel its recovery into the next strike; a whiff or a blocked strike keeps its
                // full recovery (Aura ruling C), so mashing into guard is still punished
                if (f.landed && f.IsStrike && f.State.Phase == ActionPhase.Recovery && (intent.Light || intent.Heavy))
                {
                    f.LastKind = f.State.Kind;
                    f.State = new ActionState { Kind = ActionKind.None, Phase = ActionPhase.Hold, Sequence = f.State.Sequence };
                    TryStart(f, new Intent { Light = intent.Light, Heavy = intent.Heavy }, chain: true);
                    return;
                }
                // late-recovery buffer: the press waits, commitment is never cancelled
                if (intent.AnyPress && f.State.Phase == ActionPhase.Recovery && f.State.Timing.Total - f.State.Elapsed <= T.Buffer
                    && f.State.Kind != ActionKind.Knockdown)
                {
                    f.buffered = intent;
                    f.hasBuffer = true;
                }
                return;
            }
            if (TryStart(f, intent)) return;
            if (intent.GuardHeld && f.Guard > 0)
            {
                if (f.State.Kind != ActionKind.Guard) Start(f, ActionKind.Guard, default, ActionPhase.Hold);
            }
            else if (f.State.Kind == ActionKind.Guard)
            {
                f.guardRegenWait = T.GuardRegenDelay;
                f.State.Kind = ActionKind.None;
            }
        }

        bool TryStart(FighterSim f, Intent intent, bool chain = false)
        {
            if (intent.Recover && CanRecover(f))
            {
                f.recoverWithItem = f.PrepItems > 0;
                Start(f, ActionKind.Recover, new ActionTiming { Active = f.recoverWithItem ? T.RecoverTimePrepared : T.RecoverTime }, ActionPhase.Active);
                return true;
            }
            if (intent.Throw) { LeaveGuard(f); Start(f, ActionKind.Throw, new ActionTiming { Startup = T.ReachIn, Active = T.ThrowPaired, Recovery = T.ThrowRecover }); f.State.Committed = true; return true; }
            if (intent.Heavy) { LeaveGuard(f); Start(f, ActionKind.Heavy, T.Heavy); f.State.Committed = true; Carry(f); return true; }
            if (intent.Light)
            {
                LeaveGuard(f);
                var kind = f.LastKind == ActionKind.LightL && (f.hasBuffer || chain) ? ActionKind.LightR : ActionKind.LightL;
                Start(f, kind, T.Light);
                f.State.Committed = true;
                Carry(f);
                return true;
            }
            if (intent.Step && f.stepCooldown <= 0)
            {
                LeaveGuard(f);
                Vector3 dir = intent.Move.sqrMagnitude > 0.04f ? new Vector3(intent.Move.x, 0, intent.Move.y).normalized : -f.Facing;
                Start(f, ActionKind.Step, new ActionTiming { Active = T.StepTime }, ActionPhase.Active);
                Travel(f, dir, T.StepDistance, T.StepTime);
                return true;
            }
            if (intent.Cast && f.CastCooldown <= 0)
            {
                LeaveGuard(f);
                Start(f, ActionKind.CastPlain, new ActionTiming { Startup = T.CastPrepare, Active = T.CastRelease, Recovery = T.CastRecover });
                return true;
            }
            return false;
        }

        // A strike thrown from a run carries the run into its wind-up: the motor slides the body CarryCap metres along
        // its velocity, through the world constraint. Reach grows only by that visible travel; the arm never reaches further.
        void Carry(FighterSim f)
        {
            if (!f.ranLastTick) return;
            float speed = Flat(f.Velocity).magnitude;
            if (speed > 1e-3f) Travel(f, f.Velocity, T.CarryCap, T.CarryCap / speed);
        }

        // During a strike's wind-up an unlocked fighter may turn up to RetargetMax toward its move input (Aura ruling D).
        // The contact sweep reads the same facing, so the body never swings where the rules do not. Fixed once Active
        // begins; a locked fighter keeps facing its target.
        void Retarget(FighterSim f, Intent intent)
        {
            if (!f.IsStrike || f.State.Phase != ActionPhase.Startup || f.LockTarget >= 0 || intent.Move.sqrMagnitude <= 0.04f) return;
            var turned = TurnToward(f.Facing, new Vector3(intent.Move.x, 0, intent.Move.y).normalized, T.TurnRate * Tuning.Step);
            float off = Vector3.SignedAngle(f.startFacing, turned, Vector3.up);
            f.Facing = Mathf.Abs(off) <= T.RetargetMax ? turned : Quaternion.AngleAxis(Mathf.Sign(off) * T.RetargetMax, Vector3.up) * f.startFacing;
        }

        void LeaveGuard(FighterSim f)
        {
            if (f.State.Kind == ActionKind.Guard) f.guardRegenWait = T.GuardRegenDelay;
        }

        bool CanRecover(FighterSim f)
        {
            if (f.Hp >= f.MaxHp || Time - f.lastCombatTime < T.RecoverQuietTime) return false;
            return !ThreatNear(f);
        }

        bool ThreatNear(FighterSim f)
        {
            foreach (var o in Fighters)
                if (o != f && !o.Defeated && Flat(o.Position - f.Position).magnitude < T.RecoverThreatRadius) return true;
            return false;
        }

        void Start(FighterSim f, ActionKind kind, ActionTiming timing, ActionPhase phase = ActionPhase.Startup)
        {
            f.LastKind = f.State.Kind == ActionKind.None || f.State.Kind == ActionKind.Guard ? f.LastKind : f.State.Kind;
            if (timing.Startup <= 0 && phase == ActionPhase.Startup) phase = timing.Active > 0 ? ActionPhase.Active : ActionPhase.Recovery;
            f.State = new ActionState { Kind = kind, Phase = phase, Elapsed = 0, Timing = timing, Sequence = ++sequence };
            f.hitThisAction.Clear();
            f.landed = false;
            f.startFacing = f.Facing;
            f.fistPrevValid = false;
            f.travelLeft = 0;
            f.hasBuffer = false;
            Started.Add((f.Id, f.State));
        }

        void End(FighterSim f)
        {
            var finished = f.State.Kind;
            if (finished == ActionKind.Step) f.stepCooldown = T.StepCooldown;
            if (finished == ActionKind.Recover)
            {
                f.Hp = f.MaxHp;
                if (f.recoverWithItem) f.PrepItems--;
            }
            if (finished == ActionKind.Knockdown) { Start(f, ActionKind.GetUp, new ActionTiming { Recovery = T.GetUpTime }, ActionPhase.Recovery); return; }
            f.LastKind = finished;
            f.State = new ActionState { Kind = ActionKind.None, Phase = ActionPhase.Hold, Sequence = f.State.Sequence };
            f.Charge01 = 0;
            if (f.hasBuffer)
            {
                var b = f.buffered;
                if (TryStart(f, b)) return;
            }
            f.hasBuffer = false;
        }

        // interruption without completion effects (no recover heal, no step cooldown, no buffered follow-up)
        void Cancel(FighterSim f)
        {
            f.LastKind = f.State.Kind;
            f.State = new ActionState { Kind = ActionKind.None, Phase = ActionPhase.Hold, Sequence = f.State.Sequence };
            f.hasBuffer = false;
            f.Charge01 = 0;
        }

        void Advance(FighterSim f, Intent intent, float dt)
        {
            var kind = f.State.Kind;
            if (kind == ActionKind.None || kind == ActionKind.Guard) return;
            if (kind == ActionKind.Recover && ThreatNear(f)) { Cancel(f); return; }
            float before = f.State.Elapsed;
            f.State.Elapsed += dt;
            var t = f.State.Timing;

            switch (kind)
            {
                case ActionKind.CastPlain:
                    if (f.State.Phase == ActionPhase.Startup && intent.Move.sqrMagnitude > 0.04f) { Cancel(f); return; } // moving before commitment cancels, no cooldown
                    if (before < t.Startup && f.State.Elapsed >= t.Startup)
                    {
                        f.State.Committed = true;
                        f.CastCooldown = T.CastCooldown;
                        if (f.Attuned && intent.CastHeld)
                        {
                            Start(f, ActionKind.CastHold, default, ActionPhase.Hold);
                            f.State.Committed = true;
                            f.holdTime = 0;
                            return;
                        }
                        Fire(f, false);
                    }
                    break;
                case ActionKind.CastHold:
                    f.holdTime += dt;
                    f.Charge01 = Mathf.Clamp01(f.holdTime / T.MaxHold);
                    if (!intent.CastHeld || f.holdTime >= T.MaxHold)
                    {
                        f.releaseCharged = f.holdTime >= T.ChargeThreshold;
                        Start(f, ActionKind.CastRelease, new ActionTiming { Active = T.CastRelease, Recovery = f.releaseCharged ? T.ChargedRecover : T.CastRecover }, ActionPhase.Active);
                        f.State.Committed = true;
                        Fire(f, f.releaseCharged);
                    }
                    return;
                case ActionKind.Throw:
                    if (before < t.Startup && f.State.Elapsed >= t.Startup) ResolveThrow(f);
                    break;
            }

            if (f.State.Kind != kind) return;
            t = f.State.Timing;
            if (f.State.Elapsed >= t.Total) { End(f); return; }
            f.State.Phase = f.State.Elapsed < t.Startup ? ActionPhase.Startup
                : f.State.Elapsed < t.Startup + t.Active ? ActionPhase.Active : ActionPhase.Recovery;
        }

        // ---------------- motor ----------------

        void Travel(FighterSim f, Vector3 dir, float distance, float time)
        {
            f.travelDir = Flat(dir).normalized;
            f.travelSpeed = distance / Mathf.Max(time, Tuning.Step);
            f.travelLeft = distance;
        }

        void Move(FighterSim f, Intent intent, float dt)
        {
            if (f.Defeated) return;
            Vector3 v = Vector3.zero;
            f.ranLastTick = false;
            if (f.travelLeft > 0)
            {
                float d = Mathf.Min(f.travelLeft, f.travelSpeed * dt);
                f.travelLeft -= d;
                v = f.travelDir * (d / dt);
            }
            else if (f.State.Kind == ActionKind.None || f.State.Kind == ActionKind.Guard)
            {
                var m = new Vector3(intent.Move.x, 0, intent.Move.y);
                if (m.sqrMagnitude > 1) m.Normalize();
                float speed = f.State.Kind == ActionKind.Guard ? T.GuardWalkSpeed : intent.Run ? T.RunSpeed : T.WalkSpeed;
                v = m * speed;
                f.ranLastTick = intent.Run && f.State.Kind == ActionKind.None && m.sqrMagnitude > 1e-4f;
            }
            f.Velocity = v;
            f.Position += v * dt;

            // facing: free fighters face the lock target (strafe), else turn toward their movement at a bounded rate;
            // committed actions keep their facing
            if (f.Free)
            {
                if (f.LockTarget >= 0 && f.LockTarget < Fighters.Count)
                {
                    var to = Flat(Fighters[f.LockTarget].Position - f.Position);
                    if (to.sqrMagnitude > 1e-6f) f.Facing = to.normalized;
                }
                else if (v.sqrMagnitude > 1e-4f && f.State.Kind == ActionKind.None) f.Facing = TurnToward(f.Facing, Flat(v).normalized, T.TurnRate * dt);
            }
        }

        void Separate()
        {
            for (int i = 0; i < Fighters.Count; i++)
                for (int j = i + 1; j < Fighters.Count; j++)
                {
                    var a = Fighters[i]; var b = Fighters[j];
                    var d = Flat(b.Position - a.Position);
                    float min = 2 * T.BodyRadius, len = d.magnitude;
                    if (len >= min) continue;
                    var n = len > 1e-5f ? d / len : Vector3.right;
                    var push = n * ((min - len) * 0.5f);
                    a.Position -= push;
                    b.Position += push;
                }
            // one world constraint per tick, swept from the pre-motion position, so separation never pushes through a wall
            foreach (var f in Fighters)
                f.Position = Geometry.Constrain(f.moveFrom, f.Position, T.BodyRadius);
        }

        // ---------------- contact ----------------

        void SweepStrike(FighterSim a)
        {
            if (a.Defeated || !a.IsStrike) return;
            var t = a.State.Timing;
            float activeStart = t.Startup, activeEnd = t.Startup + t.Active;
            float e = a.State.Elapsed;
            if (e < activeStart || e - Tuning.Step >= activeEnd) return;   // active window does not overlap this tick
            bool heavy = a.State.Kind == ActionKind.Heavy;
            float reach = heavy ? T.HeavyReach : T.LightReach;
            float p = Mathf.Clamp01((e - activeStart) / Mathf.Max(t.Active, 1e-4f));
            var fist = a.Position + a.Facing * (T.FistStart + p * (reach - T.FistStart));
            var from = a.fistPrevValid ? a.fistPrev : a.Position + a.Facing * T.FistStart;
            a.fistPrev = fist;
            a.fistPrevValid = true;

            FighterSim best = null;
            float bestT = 2;
            foreach (var d in Fighters)
            {
                if (d == a || d.Defeated || a.hitThisAction.Contains(d.Id) || Evading(d)) continue;
                if (Geometry.Blocked(a.Position, d.Position, 0f, out _)) continue;   // world first: nobody is hit through a wall
                float s = SegmentParam(from, fist, d.Position);
                var closest = Vector3.Lerp(from, fist, s);
                if (Flat(d.Position - closest).magnitude <= T.FistRadius + T.BodyRadius && s < bestT) { bestT = s; best = d; }
            }
            if (best == null) return;
            a.hitThisAction.Add(best.Id);
            var point = Vector3.Lerp(from, fist, bestT);
            float damage = a.Stat * (heavy ? T.HeavyFactor : T.LightFactor);
            if (Resolve(a, best, a.State.Kind, a.State.Sequence, damage, heavy ? T.GuardDrainHeavy : T.GuardDrainLight,
                heavy ? HitHeight.Mid : HitHeight.High, point, a.Facing, a.Position) == Outcome.Hit) a.landed = true;
        }

        bool Evading(FighterSim f) => f.State.Kind == ActionKind.Step && f.State.Elapsed >= T.EvadeFrom && f.State.Elapsed <= T.EvadeTo;

        bool Frontal(FighterSim defender, Vector3 sourcePosition)
        {
            var to = Flat(sourcePosition - defender.Position);
            if (to.sqrMagnitude < 1e-6f) return true;
            return Vector3.Angle(defender.Facing, to) <= T.GuardArcDegrees;
        }

        Outcome Resolve(FighterSim a, FighterSim d, ActionKind action, int seq, float damage, float drain, HitHeight height, Vector3 point, Vector3 dir, Vector3 source)
        {
            Outcome outcome;
            a.lastCombatTime = d.lastCombatTime = Time;
            if (d.State.Kind == ActionKind.Guard && d.Guard > 0 && Frontal(d, source))
            {
                d.Guard -= drain;
                if (d.Guard <= 0)
                {
                    d.Guard = 0;
                    d.guardRegenWait = T.GuardRegenDelay;
                    Start(d, ActionKind.GuardBreak, new ActionTiming { Recovery = T.GuardBreakTime }, ActionPhase.Recovery);
                    outcome = Outcome.GuardBroken;
                }
                else outcome = Outcome.Blocked;
            }
            else
            {
                outcome = Outcome.Hit;
                Damage(d, damage, dir, T.Knockback, 0.1f);
            }
            Contacts.Add(new ContactEvent { AttackerId = a.Id, DefenderId = d.Id, Action = action, ActionSequence = seq, Outcome = outcome, Height = height, Point = point, Normal = -Flat(dir).normalized, Time = Time });
            return outcome;
        }

        void Damage(FighterSim d, float amount, Vector3 dir, float push, float pushTime)
        {
            d.Hp = Mathf.Max(0, d.Hp - amount);
            if (d.Hp <= 0)
            {
                d.Defeated = true;
                d.Velocity = Vector3.zero;
                d.State = new ActionState { Kind = ActionKind.None, Phase = ActionPhase.Hold, Sequence = ++sequence };
                return;
            }
            Start(d, ActionKind.HitReact, new ActionTiming { Recovery = T.HitReactTime }, ActionPhase.Recovery);
            if (push > 0) Travel(d, dir, push, pushTime);
        }

        void ResolveThrow(FighterSim a)
        {
            FighterSim target = null;
            foreach (var d in Fighters)
            {
                if (d == a || d.Defeated) continue;
                var to = Flat(d.Position - a.Position);
                if (to.magnitude <= T.ThrowReach && Vector3.Angle(a.Facing, to) <= T.ThrowArcDegrees
                    && !Geometry.Blocked(a.Position, d.Position, 0f, out _)) { target = d; break; }
            }
            a.lastCombatTime = Time;
            if (target != null && target.State.Kind == ActionKind.Throw && target.State.Phase == ActionPhase.Startup)
            {
                // mutual throws: both stagger, neutral reset
                int seqA = a.State.Sequence, seqB = target.State.Sequence;
                Start(a, ActionKind.HitReact, new ActionTiming { Recovery = T.MutualStagger }, ActionPhase.Recovery);
                Start(target, ActionKind.HitReact, new ActionTiming { Recovery = T.MutualStagger }, ActionPhase.Recovery);
                Contacts.Add(new ContactEvent { AttackerId = a.Id, DefenderId = target.Id, Action = ActionKind.Throw, ActionSequence = seqA, Outcome = Outcome.Interrupted, Point = (a.Position + target.Position) * 0.5f, Time = Time });
                Contacts.Add(new ContactEvent { AttackerId = target.Id, DefenderId = a.Id, Action = ActionKind.Throw, ActionSequence = seqB, Outcome = Outcome.Interrupted, Point = (a.Position + target.Position) * 0.5f, Time = Time });
                return;
            }
            var landing = a.Position + a.Facing * T.ThrowLand;
            bool legal = Geometry.Fits(landing, T.BodyRadius)
                && (target == null || !Geometry.Blocked(target.Position, landing, T.BodyRadius, out _));
            // a target already striking (preparation or active) beats the reach-in: the throw fails and the strike lands
            bool striking = target != null && target.IsStrike && target.State.Phase != ActionPhase.Recovery;
            if (target == null || Evading(target) || !legal || striking)
            {
                // whiff or illegal staging: visible failed attempt, whiff recovery, nobody moves through a wall
                a.State.Timing = new ActionTiming { Startup = T.ReachIn, Active = 0, Recovery = T.ThrowWhiff };
                Contacts.Add(new ContactEvent { AttackerId = a.Id, DefenderId = target != null ? target.Id : -1, Action = ActionKind.Throw, ActionSequence = a.State.Sequence, Outcome = Outcome.Miss, Point = landing, Time = Time });
                return;
            }
            target.lastCombatTime = Time;
            int seq = a.State.Sequence;
            Contacts.Add(new ContactEvent { AttackerId = a.Id, DefenderId = target.Id, Action = ActionKind.Throw, ActionSequence = seq, Outcome = Outcome.Hit, Point = target.Position, Normal = -a.Facing, Time = Time });
            target.Hp = Mathf.Max(0, target.Hp - T.ThrowDamage);
            if (target.Hp <= 0) { target.Defeated = true; return; }
            Start(target, ActionKind.Knockdown, new ActionTiming { Recovery = T.KnockdownTime }, ActionPhase.Recovery);
            Travel(target, landing - target.Position, Flat(landing - target.Position).magnitude, T.ThrowPaired);
        }

        void Fire(FighterSim f, bool charged)
        {
            var spawn = f.Position + f.Facing * (T.BodyRadius + T.ProjectileRadius + 0.05f);
            // point-blank at a wall: start on this side so the first sweep meets the wall, never the far side
            if (Geometry.Blocked(f.Position, spawn, 0f, out var wall)) spawn = Vector3.Lerp(f.Position, spawn, Mathf.Max(0f, wall - 0.01f));
            Projectiles.Add(new Projectile
            {
                Owner = f.Id, Sequence = f.State.Sequence, Charged = charged,
                Position = spawn, Direction = f.Facing,
            });
        }

        void AdvanceProjectiles(float dt)
        {
            foreach (var pr in Projectiles)
            {
                if (!pr.Alive) continue;
                var from = pr.Position;
                var to = from + pr.Direction * (T.ProjectileSpeed * dt);
                pr.Traveled += T.ProjectileSpeed * dt;

                // world first: a wall stops the projectile before any fighter behind it
                float wallT = Geometry.Blocked(from, to, 0f, out var bt) ? bt : 2;

                FighterSim hit = null;
                float hitT = 2;
                foreach (var d in Fighters)
                {
                    if (d.Id == pr.Owner || d.Defeated || Evading(d)) continue;
                    float s = SegmentParam(from, to, d.Position);
                    var at = Vector3.Lerp(from, to, s);
                    // the projectile's radius never reaches a body on the far side of a wall it has not crossed yet
                    if (Flat(d.Position - at).magnitude <= T.ProjectileRadius + T.BodyRadius && s < hitT
                        && !Geometry.Blocked(at, d.Position, 0f, out _)) { hitT = s; hit = d; }
                }

                var owner = Fighters[pr.Owner];
                if (hit != null && hitT <= wallT)
                {
                    var point = Vector3.Lerp(from, to, hitT);
                    pr.Alive = false;
                    if (pr.Charged) Explode(owner, pr, point);
                    else Resolve(owner, hit, ActionKind.CastPlain, pr.Sequence, T.CastDamage, T.GuardDrainCast, HitHeight.Mid, point, pr.Direction, point - pr.Direction);
                }
                else if (wallT <= 1 || pr.Traveled >= T.ProjectileRange)
                {
                    var point = wallT <= 1 ? Vector3.Lerp(from, to, wallT) : to;
                    pr.Alive = false;
                    if (pr.Charged) Explode(owner, pr, point);
                    else Contacts.Add(new ContactEvent { AttackerId = pr.Owner, DefenderId = -1, Action = ActionKind.CastPlain, ActionSequence = pr.Sequence, Outcome = Outcome.Miss, Point = point, Time = Time });
                }
                else pr.Position = to;
            }
            Projectiles.RemoveAll(p => !p.Alive);
        }

        // charged fire: bounded area with falloff and push; a frontal guard takes no damage but is pushed and drained
        void Explode(FighterSim owner, Projectile pr, Vector3 center)
        {
            bool any = false;
            var origin = center - pr.Direction * 0.05f;   // off the wall surface, on the caster's side
            foreach (var d in Fighters)
            {
                if (d.Id == owner.Id || d.Defeated) continue;
                if (Geometry.Blocked(origin, d.Position, 0f, out _)) continue;   // the area never reaches through walls
                var away = Flat(d.Position - center);
                float dist = Mathf.Max(0, away.magnitude - T.BodyRadius);
                if (dist > T.ChargedRadius) continue;
                any = true;
                var dir = away.sqrMagnitude > 1e-6f ? away.normalized : pr.Direction;
                float factor = 1 - (1 - T.ChargedEdgeFactor) * (dist / T.ChargedRadius);
                owner.lastCombatTime = d.lastCombatTime = Time;
                Outcome outcome;
                if (d.State.Kind == ActionKind.Guard && d.Guard > 0 && Frontal(d, center))
                {
                    d.Guard -= T.GuardDrainCharged;
                    outcome = Outcome.Blocked;
                    if (d.Guard <= 0)
                    {
                        d.Guard = 0;
                        d.guardRegenWait = T.GuardRegenDelay;
                        Start(d, ActionKind.GuardBreak, new ActionTiming { Recovery = T.GuardBreakTime }, ActionPhase.Recovery);
                        outcome = Outcome.GuardBroken;
                    }
                    Travel(d, dir, T.ChargedPush, 0.25f);
                }
                else
                {
                    outcome = Outcome.Hit;
                    Damage(d, T.ChargedDamage * factor, dir, T.ChargedPush, 0.25f);
                }
                Contacts.Add(new ContactEvent { AttackerId = owner.Id, DefenderId = d.Id, Action = ActionKind.CastRelease, ActionSequence = pr.Sequence, Outcome = outcome, Height = HitHeight.Mid, Point = center, Normal = -dir, Time = Time });
            }
            if (!any) Contacts.Add(new ContactEvent { AttackerId = owner.Id, DefenderId = -1, Action = ActionKind.CastRelease, ActionSequence = pr.Sequence, Outcome = Outcome.Miss, Point = center, Time = Time });
        }

        // ---------------- helpers ----------------

        static Vector3 Flat(Vector3 v) => new Vector3(v.x, 0, v.z);

        // planar turn about +Y, so even an exact reversal never tips the facing out of the ground plane
        static Vector3 TurnToward(Vector3 from, Vector3 to, float maxDegrees)
        {
            float a = Vector3.SignedAngle(from, to, Vector3.up);
            return Mathf.Abs(a) <= maxDegrees ? to : Quaternion.AngleAxis(Mathf.Sign(a) * maxDegrees, Vector3.up) * from;
        }

        static float SegmentParam(Vector3 a, Vector3 b, Vector3 p)
        {
            var ab = Flat(b - a);
            float len2 = ab.sqrMagnitude;
            if (len2 < 1e-9f) return 0;
            return Mathf.Clamp01(Vector3.Dot(Flat(p - a), ab) / len2);
        }
    }
}
