using System;
using Guildbound.Contracts;
using UnityEngine;
using UnityEngine.Animations;
using UnityEngine.Playables;

namespace Guildbound.Anim
{
    // Drives Aura's existing HeroAnimator. Manual evaluation keeps source pose time tied to ActionState.
    // Never writes the simulated position, facing, action window, damage or bone lengths.
    [DefaultExecutionOrder(200)]
    public sealed class FighterAnimation : MonoBehaviour
    {
        IFighterPresentation source;
        FighterAnimationProfile profile;
        Animator animator;
        PlayableGraph graph;
        AnimatorControllerPlayable controller;
        FighterClip current, previous;
        FighterProceduralPose procedural;
        int currentHash, previousHash, lastSequence = -1;
        float blendRemaining, loopTime, guardHitAt = -10, defeatTime;
        float lastElapsed, lastRecovery, blendSpan, blendElapsed, holdSeconds;
        // Chained links: the outgoing state is held at its exit phase and the incoming clip is set by the rule each tick.
        AnimationMixerPlayable blend;
        AnimationClipPlayable overlay;
        bool chainBlend;
        bool rearHit, throwMiss, mutualBreak, defeated;

        public string PresentedState => current == null ? "" : current.state;
        public float SourceSeconds { get; private set; }
        public bool RootMotionDisabled => !animator.applyRootMotion;

        public void SetDefeated(bool value)
        {
            if (value != defeated)
            {
                defeatTime = 0;
                current = null;
                lastSequence = -1;
            }
            defeated = value;
        }

        public void Bind(IFighterPresentation fighter, Animator target, FighterAnimationProfile data)
        {
            source = fighter; animator = target; profile = data;
            if (!target.avatar.isValid || !target.avatar.isHuman)
                throw new InvalidOperationException("FighterAnimation requires a valid Humanoid Avatar.");
            target.applyRootMotion = false;
            target.runtimeAnimatorController = null;
            target.cullingMode = AnimatorCullingMode.AlwaysAnimate;
            graph = PlayableGraph.Create("Guildbound rule-timed HeroAnimator");
            graph.SetTimeUpdateMode(DirectorUpdateMode.Manual);
            controller = AnimatorControllerPlayable.Create(graph, profile.controller);
            controller.SetLayerWeight(0, 1);
            var output = AnimationPlayableOutput.Create(graph, "Fighter pose", target);
            overlay = AnimationClipPlayable.Create(graph, profile.Find("Idle").clip);
            blend = AnimationMixerPlayable.Create(graph, 2);
            blend.ConnectInput(0, controller, 0);
            blend.ConnectInput(1, overlay, 0);
            blend.SetInputWeight(0, 1);
            blend.SetInputWeight(1, 0);
            output.SetSourcePlayable(blend);
            procedural = gameObject.AddComponent<FighterProceduralPose>();
            procedural.Bind(source, target);
            source.Contact += OnContact;
            source.ActionStarted += OnStarted;
            graph.Play();
        }

        void OnStarted(ActionState state)
        {
            if (state.Kind == ActionKind.Throw) throwMiss = false;
            if (state.Kind != ActionKind.HitReact) mutualBreak = false;
        }

        void OnContact(ContactEvent contact)
        {
            if (contact.AttackerId == source.Id && contact.Action == ActionKind.Throw
                && contact.ActionSequence == source.Current.Sequence)
                throwMiss = contact.Outcome == Outcome.Miss;
            if (contact.Outcome == Outcome.Interrupted && contact.Action == ActionKind.Throw)
                mutualBreak = true;
            if (contact.DefenderId != source.Id) return;
            // The core normal points from the defender toward the incoming attacker.
            rearHit = Vector3.Dot(source.Facing, contact.Normal) < 0;
            if (contact.Outcome == Outcome.Blocked) guardHitAt = Time.time;
        }

        void LateUpdate()
        {
            if (!graph.IsValid()) return;
            ActionState action = source.Current;
            string state = Choose(action, out bool loop, out float seconds);
            bool restart = action.Sequence != lastSequence && !loop;
            bool changed = current == null || current.state != state || restart;
            // Blends advance on the rule clock (ActionState.Elapsed, fixed tick) so they cannot drift from the pose.
            // Looping states have no rule clock, so they fall back to render time.
            float ruleDelta = loop ? Time.deltaTime : Mathf.Max(0, action.Elapsed - lastElapsed);
            if (changed)
            {
                if (chainBlend)
                {
                    // A new change interrupts a live chain: settle on the current pose so nothing pops.
                    controller.Play(currentHash, 0, SourceSeconds / current.clip.length);
                    chainBlend = false;
                    blend.SetInputWeight(0, 1);
                    blend.SetInputWeight(1, 0);
                }
                previous = current;
                previousHash = currentHash;
                current = profile.Find(state);
                currentHash = Animator.StringToHash("Base Layer." + state);
                loopTime = 0;
                ruleDelta = 0;
                float offset = seconds < 0 ? current.Sample(action) : seconds;
                if (lastSequence < 0)
                {
                    controller.Play(currentHash, 0, 0);
                    blendRemaining = 0;
                }
                else
                {
                    var link = previous == null ? null : profile.Chain(previous.state, state);
                    if (link != null)
                    {
                        // Outgoing is held at its authored exit phase; the incoming clip is driven by the rule sample.
                        // A late cancel holds where the outgoing clip actually is; an early one holds at the authored phase.
                        holdSeconds = Mathf.Max(link.exitPhase, SourceSeconds);
                        controller.Play(previousHash, 0, holdSeconds / previous.clip.length);
                        overlay.Destroy();
                        overlay = AnimationClipPlayable.Create(graph, current.clip);
                        blend.ConnectInput(1, overlay, 0);
                        blend.SetInputWeight(0, 1);
                        blend.SetInputWeight(1, 0);
                        blendSpan = Mathf.Clamp(lastRecovery, profile.minBlend, link.blendSeconds);
                        blendElapsed = 0;
                        chainBlend = true;
                        blendRemaining = 0;
                    }
                    else
                    {
                        controller.CrossFadeInFixedTime(currentHash, profile.blendSeconds, 0, offset);
                        blendRemaining = profile.blendSeconds;
                    }
                }
            }
            lastElapsed = action.Elapsed;
            lastRecovery = action.Timing.Recovery;
            lastSequence = action.Sequence;
            if (defeated) defeatTime += Time.deltaTime;
            controller.SetBool("RulesIdle", !defeated && action.Kind == ActionKind.None);
            controller.SetFloat("Speed", source.Velocity.magnitude);
            controller.SetBool("Guarding", action.Kind == ActionKind.Guard);
            if (loop)
            {
                float rate = state == "WalkF" || state == "WalkB"
                    ? source.Velocity.magnitude / profile.walkReferenceSpeed : 1;
                loopTime += Time.deltaTime * rate;
                seconds = Mathf.Repeat(loopTime, current.clip.length);
            }
            else if (seconds < 0) seconds = current.Sample(action);
            SourceSeconds = Mathf.Clamp(seconds, 0, current.clip.length);
            procedural.RestoreAuthoredPose();
            if (chainBlend)
            {
                blendElapsed += ruleDelta;
                float w = Mathf.Clamp01(blendElapsed / blendSpan);
                controller.Play(previousHash, 0, holdSeconds / previous.clip.length);
                overlay.SetTime(current.Sample(action));
                blend.SetInputWeight(0, 1 - w);
                blend.SetInputWeight(1, w);
                graph.Evaluate(0);
                if (w >= 1)
                {
                    // Overlay time equals the rule sample at this frame, so handing back to the state machine is seamless.
                    chainBlend = false;
                    blend.SetInputWeight(0, 1);
                    blend.SetInputWeight(1, 0);
                    controller.Play(currentHash, 0, SourceSeconds / current.clip.length);
                    graph.Evaluate(0);
                }
            }
            else if (blendRemaining > 0)
            {
                // Non-chained blend: evaluate on the rule delta only and never hard-set normalized time.
                graph.Evaluate(ruleDelta);
                blendRemaining -= ruleDelta;
            }
            else
            {
                controller.Play(currentHash, 0, SourceSeconds / current.clip.length);
                graph.Evaluate(0);
            }
            // The imported animation root stays under the sole motor; root curves cannot accumulate travel.
            transform.localPosition = Vector3.zero;
            transform.localRotation = Quaternion.identity;
            procedural.Apply(Time.deltaTime, defeated);
        }

        string Choose(ActionState action, out bool loop, out float seconds)
        {
            loop = false; seconds = -1;
            if (defeated)
            {
                seconds = Mathf.Min(defeatTime, profile.Find("Knockdown").clip.length);
                return "Knockdown";
            }
            switch (action.Kind)
            {
                case ActionKind.None:
                    loop = true;
                    if (source.Velocity.sqrMagnitude < .01f) return "Idle";
                    return Vector3.Dot(source.Velocity, source.Facing) < -.05f ? "WalkB" : "WalkF";
                case ActionKind.LightL: return "Light1";
                case ActionKind.LightR: return "Light2";
                case ActionKind.Heavy: return "Heavy1";
                case ActionKind.Guard:
                    if (Time.time - guardHitAt < .24f)
                    {
                        seconds = Mathf.Clamp01((Time.time - guardHitAt) / .24f) * profile.Find("GuardHit").clip.length;
                        return "GuardHit";
                    }
                    loop = true; return "Guard";
                case ActionKind.Step:
                    return Vector3.Dot(source.Velocity, source.Facing) < 0 ? "StepB" : "StepF";
                case ActionKind.Throw:
                    var grab = profile.Find("GrabStart");
                    if (action.Elapsed < action.Timing.Startup)
                    {
                        seconds = Mathf.Clamp01(action.Elapsed / action.Timing.Startup) * grab.clip.length;
                        return "GrabStart";
                    }
                    if (throwMiss)
                    {
                        seconds = (1 - Mathf.Clamp01((action.Elapsed - action.Timing.Startup) / action.Timing.Recovery)) * grab.clip.length;
                        return "GrabStart";
                    }
                    seconds = Mathf.Clamp01((action.Elapsed - action.Timing.Startup)
                        / (action.Timing.Active + action.Timing.Recovery)) * profile.Find("GrabThrow").clip.length;
                    return "GrabThrow";
                case ActionKind.HitReact: return mutualBreak ? "GrabBreak" : rearHit ? "HitB" : "HitF";
                case ActionKind.GuardBreak: return "GrabBreak";
                case ActionKind.Knockdown: return "Knockdown";
                case ActionKind.GetUp: return "Getup";
                // Cast/recover clips are the next pass, not extra controller states in the first owned core subset.
                default: loop = true; return "Guard";
            }
        }

        void OnDestroy()
        {
            if (source != null) { source.Contact -= OnContact; source.ActionStarted -= OnStarted; }
            if (graph.IsValid()) graph.Destroy();
        }
    }
}
