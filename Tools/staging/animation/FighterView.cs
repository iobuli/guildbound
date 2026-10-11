using System;
using Guildbound.Anim;
using Guildbound.Combat;
using Guildbound.Contracts;
using UnityEngine;

namespace Guildbound.Play
{
    // Presentation handle for one simulated fighter (ICombatPresentation). Follows the motor; never moves itself.
    // Draws a placeholder body until Codex's animated rig binds to it.
    public class FighterView : MonoBehaviour, IFighterPresentation
    {
        public FighterSim Sim;
        public Transform lockTarget;
        public bool placeholder = true;
        public Color tint = new Color(0.35f, 0.6f, 1f);
        public FighterAnimationProfile animationProfile;

        FighterAnimation animationView;

        Transform fist, guardDisc;
        Material bodyMat;

        public int Id => Sim.Id;
        public ActionState Current => Sim.State;
        public Vector3 Position => Sim.Position;
        public Vector3 Velocity => Sim.Velocity;
        public Vector3 Facing => Sim.Facing;
        public bool Grounded => true;
        public Transform LockTarget => lockTarget;
        public float FireCharge01 => Sim.Charge01;
        public event Action<ContactEvent> Contact;
        public event Action<ActionState> ActionStarted;

        internal void RaiseContact(ContactEvent e) => Contact?.Invoke(e);
        internal void RaiseStarted(ActionState s) => ActionStarted?.Invoke(s);

        void Start()
        {
            if (!animationProfile) animationProfile = Resources.Load<FighterAnimationProfile>("FighterPresentation");
            if (animationProfile)
            {
                var rig = Instantiate(animationProfile.rig, transform);
                rig.name = "Animated Armature";
                rig.transform.localPosition = Vector3.zero;
                rig.transform.localRotation = Quaternion.identity;
                foreach (var collider in rig.GetComponentsInChildren<Collider>()) Destroy(collider);
                var animator = rig.GetComponent<Animator>();
                animationView = rig.AddComponent<FighterAnimation>();
                animationView.Bind(this, animator, animationProfile);
                placeholder = false;
            }
            if (!placeholder) return;
            var body = Part(PrimitiveType.Capsule, "Body", new Vector3(0, 0.9f, 0), new Vector3(0.6f, 0.9f, 0.6f));
            bodyMat = body.GetComponent<Renderer>().material;
            bodyMat.color = tint;
            Part(PrimitiveType.Cube, "Nose", new Vector3(0, 1.5f, 0.3f), new Vector3(0.15f, 0.1f, 0.2f)).GetComponent<Renderer>().material.color = Color.white;
            fist = Part(PrimitiveType.Sphere, "Fist", new Vector3(0, 1.3f, 0.2f), Vector3.one * 0.2f).transform;
            guardDisc = Part(PrimitiveType.Cylinder, "GuardDisc", new Vector3(0, 1.2f, 0.35f), new Vector3(0.9f, 0.02f, 0.9f)).transform;
            guardDisc.localRotation = Quaternion.Euler(90, 0, 0);
            var gm = guardDisc.GetComponent<Renderer>().material;
            gm.color = new Color(0.37f, 0.84f, 1f);
        }

        GameObject Part(PrimitiveType type, string name, Vector3 pos, Vector3 scale)
        {
            var go = GameObject.CreatePrimitive(type);
            Destroy(go.GetComponent<Collider>());   // the owned motor is the only mover and collider authority
            go.name = name;
            go.transform.SetParent(transform, false);
            go.transform.localPosition = pos;
            go.transform.localScale = scale;
            return go;
        }

        void LateUpdate()
        {
            if (Sim == null) return;
            transform.position = Sim.Position;
            if (Sim.Facing.sqrMagnitude > 1e-6f) transform.rotation = Quaternion.LookRotation(Sim.Facing, Vector3.up);
            if (animationView) animationView.SetDefeated(Sim.Defeated);
            if (!placeholder || fist == null) return;

            var s = Sim.State;
            bool strike = s.Kind == ActionKind.LightL || s.Kind == ActionKind.LightR || s.Kind == ActionKind.Heavy;
            float reach = 0.2f;
            if (strike)
            {
                var t = s.Timing;
                float full = s.Kind == ActionKind.Heavy ? 0.95f : 0.8f;
                float p = s.Elapsed < t.Startup ? -0.15f * (s.Elapsed / Mathf.Max(t.Startup, 1e-3f))
                    : s.Elapsed < t.Startup + t.Active ? (s.Elapsed - t.Startup) / Mathf.Max(t.Active, 1e-3f)
                    : 1f - (s.Elapsed - t.Startup - t.Active) / Mathf.Max(t.Recovery, 1e-3f);
                reach = 0.2f + Mathf.Max(p, -0.15f) * (full - 0.2f);
            }
            fist.localPosition = new Vector3(s.Kind == ActionKind.LightR ? -0.15f : 0.15f, s.Kind == ActionKind.Heavy ? 1.0f : 1.35f, reach);
            guardDisc.gameObject.SetActive(s.Kind == ActionKind.Guard);
            var c = Sim.Defeated ? Color.gray : s.Kind == ActionKind.HitReact ? Color.white : s.Kind == ActionKind.GuardBreak ? new Color(1f, 0.7f, 0.28f)
                : s.Kind == ActionKind.CastHold ? Color.Lerp(tint, new Color(1f, 0.5f, 0.1f), Sim.Charge01) : tint;
            bodyMat.color = c;
            transform.localScale = Sim.Defeated ? new Vector3(1f, 0.3f, 1f) : Vector3.one;
        }
    }
}
