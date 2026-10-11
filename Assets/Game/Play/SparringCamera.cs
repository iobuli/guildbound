using Guildbound.Contracts;
using UnityEngine;

namespace Guildbound.Play
{
    // Frames the motor truth from a spring-following shoulder. Only camera offsets lag or shake.
    [DefaultExecutionOrder(250)]
    public class SparringCamera : MonoBehaviour
    {
        public SparringDriver driver;
        public float distance = 5.5f, height = 2.6f, lookHeight = 1.1f, follow = 6f;
        public float shoulderOffset = 2.6f, explorationDistance = 6.8f, explorationHeight = 3.4f;
        [Range(0, 1)] public float shakeStrength = 1;
        Vector3 position, positionVelocity, pulse, pulseVelocity;
        Quaternion rotation;
        FighterView boundPlayer, boundPartner;
        ContactEvent lastContact;
        bool initialized, hasContact;
        int sequence;

        public float ImpactOffset => pulse.magnitude;
        public bool CombatFraming { get; private set; }

        void LateUpdate()
        {
            if (driver == null || driver.World == null || !driver.PlayerView || !driver.OpponentView) return;
            BindContacts();
            var player = driver.Player;
            var opponent = driver.Opponent;
            var p = player.Position;
            var o = opponent.Position;
            CombatFraming = !opponent.Defeated && (player.LockTarget >= 0 || player.IsStrike
                || player.State.Kind == ActionKind.Guard || player.State.Kind == ActionKind.Throw);
            var dir = CombatFraming ? o - p : player.Facing;
            dir.y = 0;
            if (dir.sqrMagnitude < .0001f) dir = player.Facing;
            dir.Normalize();
            var side = Vector3.Cross(Vector3.up, dir);
            float separation = Vector3.Distance(p, o);
            float closeRange = distance * Mathf.Lerp(.75f, 1, Mathf.InverseLerp(.8f, 3, separation));
            float range = CombatFraming ? Mathf.Min(9.5f, closeRange + Mathf.Max(0, separation - 3) * .45f) : explorationDistance;
            var target = p - dir * range + Vector3.up * (CombatFraming ? height : explorationHeight)
                + side * (CombatFraming ? shoulderOffset : .35f);
            var focus = (CombatFraming ? Vector3.Lerp(p, o, .45f) : p) + Vector3.up * lookHeight
                + Vector3.ClampMagnitude(player.Velocity, 4) * .09f;
            float dt = Mathf.Clamp(Time.deltaTime, 0, .05f);
            if (!initialized || player.State.Sequence < sequence)
            {
                position = target;
                positionVelocity = pulse = pulseVelocity = Vector3.zero;
                rotation = Quaternion.LookRotation(focus - position, Vector3.up);
                initialized = true;
                hasContact = false;
            }
            sequence = player.State.Sequence;
            position = Vector3.SmoothDamp(position, target, ref positionVelocity, 1 / Mathf.Max(1, follow), Mathf.Infinity, dt);
            rotation = Quaternion.Slerp(rotation, Quaternion.LookRotation(focus - position, Vector3.up), 1 - Mathf.Exp(-follow * dt));
            float decay = Mathf.Exp(-24 * dt);
            var c = pulseVelocity + pulse * 24;
            pulse = (pulse + c * dt) * decay;
            pulseVelocity = (pulseVelocity - c * (24 * dt)) * decay;
            pulse = Vector3.ClampMagnitude(pulse, .04f);
            transform.SetPositionAndRotation(position + pulse * shakeStrength, rotation);
        }

        void BindContacts()
        {
            if (boundPlayer == driver.PlayerView && boundPartner == driver.OpponentView) return;
            Unbind();
            boundPlayer = driver.PlayerView;
            boundPartner = driver.OpponentView;
            boundPlayer.Contact += OnContact;
            boundPartner.Contact += OnContact;
        }

        void OnContact(ContactEvent contact)
        {
            if (contact.Outcome != Outcome.Hit && contact.Outcome != Outcome.Blocked && contact.Outcome != Outcome.GuardBroken) return;
            // Dispatch sends the same resolved event to attacker and defender. One camera impulse per event.
            if (hasContact && contact.Time == lastContact.Time && contact.ActionSequence == lastContact.ActionSequence
                && contact.AttackerId == lastContact.AttackerId && contact.DefenderId == lastContact.DefenderId
                && contact.Outcome == lastContact.Outcome) return;
            hasContact = true; lastContact = contact;
            float force = contact.Action == ActionKind.Heavy ? 1.3f : .7f;
            if (contact.Outcome == Outcome.Blocked) force *= .35f;
            pulseVelocity += (contact.Normal.normalized + Vector3.up * .16f) * force;
            pulseVelocity = Vector3.ClampMagnitude(pulseVelocity, 2);
        }

        void Unbind()
        {
            if (boundPlayer) boundPlayer.Contact -= OnContact;
            if (boundPartner) boundPartner.Contact -= OnContact;
            boundPlayer = boundPartner = null;
        }

        void OnDisable()
        {
            Unbind();
            initialized = false;
            hasContact = false;
        }
    }
}
