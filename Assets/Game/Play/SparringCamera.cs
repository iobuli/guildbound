using UnityEngine;

namespace Guildbound.Play
{
    // Minimal third-person framing for the spike: behind the player, looking at the midpoint so both fighters'
    // preparation stays legible. Codex's Cam work replaces this.
    public class SparringCamera : MonoBehaviour
    {
        public SparringDriver driver;
        public float distance = 5.5f, height = 2.6f, lookHeight = 1.1f, follow = 6f;

        void LateUpdate()
        {
            if (driver == null || driver.World == null) return;
            var p = driver.Player.Position;
            var o = driver.Opponent.Position;
            var dir = o - p; dir.y = 0;
            if (dir.sqrMagnitude < 1e-4f) dir = Vector3.forward;
            dir.Normalize();
            var mid = (p + o) * 0.5f;
            var target = p - dir * distance + Vector3.up * height;
            transform.position = Vector3.Lerp(transform.position, target, 1f - Mathf.Exp(-follow * Time.deltaTime));
            transform.rotation = Quaternion.LookRotation(mid + Vector3.up * lookHeight - transform.position, Vector3.up);
        }
    }
}
