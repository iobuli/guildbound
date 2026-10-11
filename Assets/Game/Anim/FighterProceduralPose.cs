using Guildbound.Contracts;
using UnityEngine;

namespace Guildbound.Anim
{
    // Small additive layer after the owned clips. Restore before graph evaluation to prevent accumulation.
    // This bends existing bones only; root, bone translations/lengths and simulation stay unchanged.
    public sealed class FighterProceduralPose : MonoBehaviour
    {
        public float maxLeanDegrees = 15;
        public float recoilLimitDegrees = 8;
        public float recoilFrequency = 20;
        IFighterPresentation source;
        Transform hips, spine, chest;
        Transform[] bones;
        Quaternion[] authored;
        Vector3 previousVelocity, previousPosition;
        Vector2 lean, recoil, recoilVelocity;
        float hipHeading, spineHeading, chestHeading, hipRate, spineRate, chestRate;
        float breathing;
        int sequence;
        bool captured;
        bool hasContact;
        ContactEvent lastContact;

        public float LeanDegrees => lean.magnitude;
        public float RecoilDegrees => recoil.magnitude;
        public float TurnLagDegrees { get; private set; }

        public void Bind(IFighterPresentation fighter, Animator animator)
        {
            source = fighter;
            hips = animator.GetBoneTransform(HumanBodyBones.Hips);
            spine = animator.GetBoneTransform(HumanBodyBones.Spine);
            chest = animator.GetBoneTransform(HumanBodyBones.Chest);
            if (!chest) chest = spine;
            bones = chest == spine ? new[] { hips, spine } : new[] { hips, spine, chest };
            authored = new Quaternion[bones.Length];
            ResetMotion();
            source.Contact += OnContact;
        }

        void ResetMotion()
        {
            hipHeading = spineHeading = chestHeading = Heading(source.Facing);
            hipRate = spineRate = chestRate = 0;
            lean = recoil = recoilVelocity = Vector2.zero;
            previousVelocity = source.Velocity;
            previousPosition = source.Position;
            sequence = source.Current.Sequence;
            hasContact = false;
        }

        public void RestoreAuthoredPose()
        {
            if (!captured) return;
            for (int i = 0; i < bones.Length; i++) if (bones[i]) bones[i].localRotation = authored[i];
            captured = false;
        }

        public void Apply(float deltaTime, bool defeated = false)
        {
            if (source == null) return;
            for (int i = 0; i < bones.Length; i++) authored[i] = bones[i].localRotation;
            captured = true;
            float dt = Mathf.Clamp(deltaTime, 0, .05f);
            if ((source.Position - previousPosition).sqrMagnitude > 4
                || source.Current.Sequence < sequence) ResetMotion();
            sequence = source.Current.Sequence;
            previousPosition = source.Position;
            var action = source.Current;
            bool groundedPose = !defeated && source.Grounded && action.Kind != ActionKind.Knockdown && action.Kind != ActionKind.GetUp;
            bool strike = action.Kind == ActionKind.LightL || action.Kind == ActionKind.LightR || action.Kind == ActionKind.Heavy;
            Vector3 localVelocity = transform.InverseTransformDirection(source.Velocity);
            Vector3 accel = dt > .0001f ? (source.Velocity - previousVelocity) / dt : Vector3.zero;
            previousVelocity = source.Velocity;
            Vector3 localAccel = transform.InverseTransformDirection(Vector3.ClampMagnitude(accel, 12));
            float carry = strike && action.Phase == ActionPhase.Startup ? Mathf.Clamp01(localVelocity.z / 4) * 9 : 0;
            var targetLean = groundedPose ? new Vector2(localVelocity.z * 1.2f + localAccel.z * .32f + carry,
                -localVelocity.x * 1.4f - localAccel.x * .32f) : Vector2.zero;
            targetLean = Vector2.ClampMagnitude(targetLean, maxLeanDegrees);
            lean = Vector2.Lerp(lean, targetLean, 1 - Mathf.Exp(-12 * dt));
            // Exact critically damped spring, stable across 30/60/120 render frames.
            StepSpring(ref recoil, ref recoilVelocity, recoilFrequency, dt);
            recoil = Vector2.ClampMagnitude(recoil, recoilLimitDegrees);

            float heading = Heading(source.Facing);
            hipHeading = Mathf.SmoothDampAngle(hipHeading, heading, ref hipRate, .045f, Mathf.Infinity, dt);
            spineHeading = Mathf.SmoothDampAngle(spineHeading, hipHeading, ref spineRate, .075f, Mathf.Infinity, dt);
            chestHeading = Mathf.SmoothDampAngle(chestHeading, spineHeading, ref chestRate, .09f, Mathf.Infinity, dt);
            float hipLag = groundedPose ? Mathf.Clamp(Mathf.DeltaAngle(heading, hipHeading), -7, 7) : 0;
            float spineLag = groundedPose ? Mathf.Clamp(Mathf.DeltaAngle(hipHeading, spineHeading), -5, 5) : 0;
            float chestLag = groundedPose ? Mathf.Clamp(Mathf.DeltaAngle(spineHeading, chestHeading), -3, 3) : 0;
            TurnLagDegrees = hipLag + spineLag + chestLag;
            breathing += dt * (1.4f + Mathf.Clamp01(source.Velocity.magnitude / 4));
            float breathe = groundedPose && !strike ? Mathf.Sin(breathing * Mathf.PI * 2) * .35f : 0;
            var body = groundedPose ? lean + recoil : Vector2.zero;
            Rotate(hips, body * .15f, hipLag);
            Rotate(spine, body * .35f + new Vector2(breathe, 0), spineLag);
            if (chest != spine) Rotate(chest, body * .50f + new Vector2(breathe * .5f, 0), chestLag);
        }

        void Rotate(Transform bone, Vector2 angles, float yaw)
        {
            bone.rotation = Quaternion.AngleAxis(yaw, Vector3.up)
                * Quaternion.AngleAxis(angles.x, transform.right)
                * Quaternion.AngleAxis(angles.y, transform.forward) * bone.rotation;
        }

        void OnContact(ContactEvent contact)
        {
            if (contact.Outcome != Outcome.Hit && contact.Outcome != Outcome.Blocked && contact.Outcome != Outcome.GuardBroken) return;
            if (hasContact && contact.Time == lastContact.Time && contact.ActionSequence == lastContact.ActionSequence
                && contact.AttackerId == lastContact.AttackerId && contact.DefenderId == lastContact.DefenderId
                && contact.Outcome == lastContact.Outcome) return;
            hasContact = true; lastContact = contact;
            bool defender = contact.DefenderId == source.Id;
            if (!defender && contact.AttackerId != source.Id) return;
            var normal = transform.InverseTransformDirection(contact.Normal);
            float sign = defender ? -1 : 1;
            float strength = contact.Action == ActionKind.Heavy ? 160 : 95;
            if (!defender) strength *= .40f;
            if (contact.Outcome == Outcome.Blocked) strength *= .45f;
            recoilVelocity += new Vector2(normal.z, -normal.x) * sign * strength;
            recoilVelocity = Vector2.ClampMagnitude(recoilVelocity, 220);
        }

        public static void StepSpring(ref Vector2 position, ref Vector2 velocity, float frequency, float dt)
        {
            float decay = Mathf.Exp(-frequency * dt);
            var c = velocity + position * frequency;
            position = (position + c * dt) * decay;
            velocity = (velocity - c * frequency * dt) * decay;
        }

        static float Heading(Vector3 forward) => Mathf.Atan2(forward.x, forward.z) * Mathf.Rad2Deg;
        void OnDestroy()
        {
            RestoreAuthoredPose();
            if (source != null) source.Contact -= OnContact;
        }
    }
}
