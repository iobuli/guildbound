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
        // Phase a chain enters this clip at when its row leaves entryPhase at -1 (seconds into the clip).
        public float entry;

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

    // One declarative link: leaving `from` at its exitPhase, entering `to` at entryPhase (-1 = the clip's entry).
    // blendSeconds is the cap; the actual blend is the outgoing recovery window clamped to [minBlend, blendSeconds].
    [Serializable]
    public sealed class FighterChain
    {
        public string from, to;
        public float exitPhase, entryPhase = -1, blendSeconds = .12f;
    }

    public sealed class FighterAnimationProfile : ScriptableObject
    {
        public GameObject rig;
        public RuntimeAnimatorController controller;
        public FighterClip[] clips;
        public FighterChain[] chains = Array.Empty<FighterChain>();
        public float blendSeconds = .07f;
        public float minBlend = .03f;
        public float walkReferenceSpeed = 2.1f;

        public FighterClip Find(string state)
        {
            foreach (var entry in clips) if (entry.state == state) return entry;
            throw new InvalidOperationException("Missing presentation state: " + state);
        }

        public FighterChain Chain(string from, string to)
        {
            foreach (var link in chains) if (link.from == from && link.to == to) return link;
            return null;
        }
    }
}
