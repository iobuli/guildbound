using System;
using Guildbound.Contracts;
using UnityEngine;

namespace Guildbound.Anim
{
    [Serializable]
    public sealed class FighterClip
    {
        public string state;
        public AnimationClip clip;
        public float contact, followThrough;

        // Source pose markers fit to rule-owned phases. These do not authorize damage.
        public float Sample(ActionState action)
        {
            float t = Mathf.Max(0, action.Elapsed);
            var window = action.Timing;
            if (window.Startup > 0 && t < window.Startup)
                return Mathf.Lerp(0, contact, t / window.Startup);
            if (window.Active > 0 && t < window.Startup + window.Active)
                return Mathf.Lerp(contact, followThrough, (t - window.Startup) / window.Active);
            if (window.Recovery > 0)
                return Mathf.Lerp(followThrough, clip.length,
                    (t - window.Startup - window.Active) / window.Recovery);
            return clip.length;
        }
    }

    public sealed class FighterAnimationProfile : ScriptableObject
    {
        public GameObject rig;
        public RuntimeAnimatorController controller;
        public FighterClip[] clips;
        public float blendSeconds = .07f;
        public float walkReferenceSpeed = 2.1f;

        public FighterClip Find(string state)
        {
            foreach (var entry in clips) if (entry.state == state) return entry;
            throw new InvalidOperationException("Missing presentation state: " + state);
        }
    }
}
