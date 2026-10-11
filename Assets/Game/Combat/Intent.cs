using UnityEngine;

namespace Guildbound.Combat
{
    // One tick of player or AI intent. Pressed flags are edges and are consumed by exactly one tick.
    public struct Intent
    {
        public Vector2 Move;         // world XZ, magnitude <= 1
        public bool Run;
        public bool GuardHeld, CastHeld;
        public bool Light, Heavy, Step, Throw, Cast, Recover;

        public bool AnyPress => Light || Heavy || Step || Throw || Cast || Recover;

        public Intent WithoutPresses()
        {
            var i = this;
            i.Light = i.Heavy = i.Step = i.Throw = i.Cast = i.Recover = false;
            return i;
        }
    }
}
