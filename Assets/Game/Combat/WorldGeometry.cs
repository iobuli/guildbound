using UnityEngine;

namespace Guildbound.Combat
{
    // The static world the motor moves through (GDD S4: the collision motor is the only mover; world obstruction
    // stops contact before any actor behind it). Positions are on the XZ plane; the ground is flat at y = 0 in M1.
    public interface IWorldGeometry
    {
        // Where a body of this radius ends up when it moves from -> to: it stops at walls and slides along them.
        Vector3 Constrain(Vector3 from, Vector3 to, float radius);

        // First world contact of a body of this radius swept from -> to; t is the fraction of the segment (0..1).
        bool Blocked(Vector3 from, Vector3 to, float radius, out float t);

        // A body of this radius fits here without overlapping the world.
        bool Fits(Vector3 at, float radius);
    }

    // The sparring floor: an open square centred on the origin. Reproduces the M0.5 arena maths exactly.
    public class BoxArena : IWorldGeometry
    {
        public readonly float HalfSize;

        public BoxArena(float halfSize) => HalfSize = halfSize;

        public Vector3 Constrain(Vector3 from, Vector3 to, float radius)
        {
            float half = HalfSize - radius;
            return new Vector3(Mathf.Clamp(to.x, -half, half), 0, Mathf.Clamp(to.z, -half, half));
        }

        public bool Blocked(Vector3 from, Vector3 to, float radius, out float t)
        {
            float half = HalfSize - radius;
            t = 2;
            if (Mathf.Abs(to.x) <= half && Mathf.Abs(to.z) <= half) return false;
            float tx = to.x != from.x ? (Mathf.Sign(to.x) * half - from.x) / (to.x - from.x) : 2;
            float tz = to.z != from.z ? (Mathf.Sign(to.z) * half - from.z) / (to.z - from.z) : 2;
            if (tx < 0 || tx > 1) tx = 2;
            if (tz < 0 || tz > 1) tz = 2;
            t = Mathf.Min(tx, tz);
            return t <= 1;
        }

        public bool Fits(Vector3 at, float radius)
        {
            float half = HalfSize - radius;
            return Mathf.Abs(at.x) <= half && Mathf.Abs(at.z) <= half;
        }
    }
}
