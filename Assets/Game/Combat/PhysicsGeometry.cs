using UnityEngine;

namespace Guildbound.Combat
{
    // IWorldGeometry over Unity colliders, for the hall and the routes. M1 ground is flat (y = 0), so every query runs
    // on one horizontal plane at QueryHeight: walls and props reaching above (QueryHeight - radius) block, the floor
    // never does. Static colliders only; call Physics.SyncTransforms after toggling one (a repaired gate).
    public class PhysicsGeometry : IWorldGeometry
    {
        const float Skin = 0.01f;
        readonly int mask;
        public readonly float QueryHeight;

        public PhysicsGeometry(int layerMask, float queryHeight = 1f)
        {
            mask = layerMask;
            QueryHeight = queryHeight;
        }

        Vector3 Up(Vector3 p) => new Vector3(p.x, QueryHeight, p.z);

        public Vector3 Constrain(Vector3 from, Vector3 to, float radius)
        {
            var pos = Up(from);
            var target = Up(to);
            for (int i = 0; i < 3; i++)   // move, then slide along at most two walls (a corner stops both)
            {
                var d = target - pos;
                float len = d.magnitude;
                if (len < 1e-5f) break;
                var dir = d / len;
                if (!Physics.SphereCast(pos, radius, dir, out var hit, len + Skin, mask, QueryTriggerInteraction.Ignore))
                {
                    pos = target;
                    break;
                }
                pos += dir * Mathf.Max(0f, hit.distance - Skin);
                var n = new Vector3(hit.normal.x, 0f, hit.normal.z);
                if (n.sqrMagnitude < 1e-6f) break;
                n.Normalize();
                var rest = target - pos;
                target = pos + rest - n * Mathf.Min(0f, Vector3.Dot(rest, n));   // drop only the into-wall part
            }
            return new Vector3(pos.x, 0f, pos.z);
        }

        public bool Blocked(Vector3 from, Vector3 to, float radius, out float t)
        {
            t = 2;
            var a = Up(from);
            var d = Up(to) - a;
            float len = d.magnitude;
            if (len < 1e-5f) return false;
            RaycastHit hit;
            bool any = radius > 0f
                ? Physics.SphereCast(a, radius, d / len, out hit, len, mask, QueryTriggerInteraction.Ignore)
                : Physics.Raycast(a, d / len, out hit, len, mask, QueryTriggerInteraction.Ignore);
            if (!any) return false;
            t = hit.distance / len;
            return true;
        }

        public bool Fits(Vector3 at, float radius) => !Physics.CheckSphere(Up(at), radius, mask, QueryTriggerInteraction.Ignore);
    }
}
