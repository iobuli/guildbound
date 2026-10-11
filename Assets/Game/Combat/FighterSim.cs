using System.Collections.Generic;
using Guildbound.Contracts;
using UnityEngine;

namespace Guildbound.Combat
{
    // One fighter's rule state. Positions are on the XZ plane (y = 0); the motor in CombatWorld is the only mover.
    public class FighterSim
    {
        public readonly int Id;
        public Vector3 Position, Velocity;
        public Vector3 Facing = Vector3.forward;
        public float Hp, MaxHp, Stat = 6f, Guard;
        public bool Attuned, Defeated;
        public int PrepItems;
        public float CastCooldown, Charge01;
        public int LockTarget = -1;
        public ActionState State;
        public ActionKind LastKind;

        internal float guardRegenWait, stepCooldown, holdTime;
        internal double lastCombatTime = -1e9;
        internal Vector3 travelDir;
        internal float travelSpeed, travelLeft;
        internal bool hasBuffer, releaseCharged, recoverWithItem;
        internal Intent buffered;
        internal Vector3 fistPrev;
        internal bool fistPrevValid;
        internal readonly HashSet<int> hitThisAction = new HashSet<int>();

        public FighterSim(int id, Vector3 position, Vector3 facing, float hp, float guardMax)
        {
            Id = id;
            Position = position;
            Facing = facing.normalized;
            Hp = MaxHp = hp;
            Guard = guardMax;
        }

        public bool Free => !Defeated && (State.Kind == ActionKind.None || State.Kind == ActionKind.Guard);
        public bool IsStrike => State.Kind == ActionKind.LightL || State.Kind == ActionKind.LightR || State.Kind == ActionKind.Heavy;
    }

    public class Projectile
    {
        public int Owner, Sequence;
        public Vector3 Position, Direction;
        public bool Charged, Alive = true;
        public float Traveled;
    }
}
