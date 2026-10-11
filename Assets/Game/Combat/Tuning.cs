using Guildbound.Contracts;

namespace Guildbound.Combat
{
    // M0.5 tuning table. Every value is a starting hypothesis (H) from GDD v5.1 section 4 until measured.
    public class Tuning
    {
        public const float Step = 1f / 120f;    // fixed simulation step (s)

        // bodies and movement
        public float BodyRadius = 0.30f;
        public float WalkSpeed = 2.1f, RunSpeed = 4.0f, GuardWalkSpeed = 1.2f;
        public float ArenaHalfSize = 6f;
        public float TurnRate = 720f;           // deg/s an unlocked free fighter turns toward its movement

        // strikes: timing, reach from body centre, damage = stat x factor
        public ActionTiming Light = new ActionTiming { Startup = 0.25f, Active = 0.10f, Recovery = 0.30f };
        public ActionTiming Heavy = new ActionTiming { Startup = 0.55f, Active = 0.10f, Recovery = 0.50f };
        public float LightReach = 0.80f, HeavyReach = 0.95f, FistRadius = 0.10f, FistStart = 0.20f;
        public float LightFactor = 1f, HeavyFactor = 2f, Knockback = 0.15f;
        public float Buffer = 0.25f;
        public float CarryCap = 0.30f;          // a strike thrown from a run slides this far in its wind-up
        public float RetargetMax = 15f;         // deg an unlocked strike may turn toward move input during its wind-up

        // guard
        public float GuardMax = 100f, GuardDrainLight = 15f, GuardDrainHeavy = 45f, GuardDrainCast = 15f, GuardDrainCharged = 30f;
        public float GuardRegenDelay = 0.8f, GuardRegenRate = 25f, GuardArcDegrees = 70f;
        public float GuardBreakTime = 1.0f, HitReactTime = 0.40f;

        // step
        public float StepTime = 0.35f, StepDistance = 1.2f, EvadeFrom = 0.08f, EvadeTo = 0.22f, StepCooldown = 0.4f;

        // throw
        public float ThrowReach = 1.2f, ThrowArcDegrees = 60f, ReachIn = 0.30f, ThrowPaired = 0.80f, ThrowRecover = 0.40f;
        public float ThrowWhiff = 0.60f, ThrowLand = 1.4f, ThrowDamage = 8f, MutualStagger = 0.60f;
        public float KnockdownTime = 1.0f, GetUpTime = 0.6f;

        // cast (plain, charged after attunement)
        public float CastPrepare = 0.45f, CastRelease = 0.15f, CastRecover = 0.40f, ChargedRecover = 0.55f;
        public float CastCooldown = 4.0f, ChargeThreshold = 0.15f, MaxHold = 2.0f;
        public float ProjectileSpeed = 14f, ProjectileRadius = 0.25f, ProjectileRange = 18f;
        public float CastDamage = 10f, ChargedDamage = 14f, ChargedRadius = 1.5f, ChargedPush = 1.0f, ChargedEdgeFactor = 0.5f;

        // recover (out of combat only)
        public float RecoverTime = 3.0f, RecoverTimePrepared = 1.0f, RecoverThreatRadius = 12f, RecoverQuietTime = 5f;
    }
}
