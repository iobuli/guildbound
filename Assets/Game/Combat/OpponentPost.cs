using UnityEngine;

namespace Guildbound.Combat
{
    // M1 routes (GDD S4): an opponent holds a post, engages a foe it can see within WatchRadius, and walks home once the
    // chase takes it past LeashRadius, ignoring the foe until it is back. So a route can be passed without fighting.
    // A sparring partner has no post. Lost HP is kept on the way home (no regen). Numbers (H).
    public class OpponentPost
    {
        public readonly Vector3 Home;
        public float WatchRadius = 12f, LeashRadius = 20f, HomeTolerance = 0.5f;
        public bool Engaged { get; private set; }
        public bool Returning { get; private set; }

        public OpponentPost(Vector3 home) => Home = new Vector3(home.x, 0f, home.z);

        // Once per tick, before the pattern decides. True: fight. False: `home` is the move intent (toward the post,
        // or zero while standing at it).
        public bool Update(CombatWorld world, FighterSim me, FighterSim foe, out Vector2 home)
        {
            home = Vector2.zero;
            var fromHome = me.Position - Home;
            fromHome.y = 0f;
            if (Returning && fromHome.magnitude <= HomeTolerance) Returning = false;
            if (Engaged && fromHome.magnitude > LeashRadius) { Engaged = false; Returning = true; }
            if (!Engaged && !Returning && !foe.Defeated)
            {
                var to = foe.Position - me.Position;
                to.y = 0f;
                Engaged = to.magnitude <= WatchRadius && !world.Geometry.Blocked(me.Position, foe.Position, 0f, out _);
            }
            if (Engaged) return true;
            if (Returning) home = new Vector2(-fromHome.x, -fromHome.z).normalized;
            return false;
        }
    }
}
