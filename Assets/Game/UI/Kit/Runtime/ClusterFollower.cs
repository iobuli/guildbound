using UnityEngine;

namespace Guildbound.UI
{
    // A panel drawn across several layers (plate L1, icon L3, text L4) is shown/hidden as one: the view toggles
    // this object and the same-named cluster rects on the other layers follow.
    public class ClusterFollower : MonoBehaviour
    {
        public GameObject[] others;

        void OnEnable() { foreach (var o in others) if (o) o.SetActive(true); }
        void OnDisable() { foreach (var o in others) if (o) o.SetActive(false); }
    }
}
