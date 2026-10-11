using System;
using UnityEngine;

namespace Guildbound.Contracts
{
    public enum ActionKind { None, LightL, LightR, Heavy, Guard, Step, Throw, CastPlain, CastHold, CastRelease, Recover, HitReact, GuardBreak, Knockdown, GetUp }
    public enum ActionPhase { Startup, Active, Recovery, Hold }
    public enum Outcome { Hit, Blocked, GuardBroken, Miss, Interrupted }
    public enum HitHeight { High, Mid, Low }

    // Timing is owned by the rules (tuning table, (H) values until measured). Animation retimes its clip so the
    // clip's authored contact key lands at Startup end; it never lengthens or shortens the rule window.
    [Serializable]
    public struct ActionTiming
    {
        public float Startup, Active, Recovery;
        public float Total => Startup + Active + Recovery;
    }

    public struct ActionState
    {
        public ActionKind Kind;
        public ActionPhase Phase;
        public float Elapsed;        // seconds since the action started (fixed-step time)
        public ActionTiming Timing;
        public int Sequence;         // increments on every new action, so presentation can detect restarts
        public bool Committed;       // past the commitment point: cannot be cancelled
    }

    // Resolved by the rules from swept limb volumes; presentation shows it, never decides it.
    public struct ContactEvent
    {
        public int AttackerId, DefenderId;
        public ActionKind Action;
        public int ActionSequence;   // ActionState.Sequence of the attacking action, so a late visual hold binds to the right swing
        public Outcome Outcome;
        public HitHeight Height;
        public Vector3 Point, Normal;
        public double Time;
    }

    // What Anim/VFX/Camera read from one fighter. The motor is the only mover: root motion is ignored and the
    // pose layer fits feet/hands to the motor's position within the fixed-length rig (no bone stretch).
    public interface IFighterPresentation
    {
        int Id { get; }
        ActionState Current { get; }
        Vector3 Position { get; }
        Vector3 Velocity { get; }
        Vector3 Facing { get; }
        bool Grounded { get; }
        Transform LockTarget { get; }        // null when no target
        float FireCharge01 { get; }
        event Action<ContactEvent> Contact;  // raised once per resolved contact, after the outcome is final
        event Action<ActionState> ActionStarted;
    }
}
