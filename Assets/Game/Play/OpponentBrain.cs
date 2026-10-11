using Guildbound.Combat;
using Guildbound.Contracts;
using UnityEngine;

namespace Guildbound.Play
{
    // Sparring partner patterns (GDD v5.1 section 4). Deterministic: decisions depend only on time and state.
    // Pressure: approaches, clearly prepares a two-hit attempt, pauses, resets; punishes casts and throw attempts.
    // Defensive: visibly guards, backs off after a failed throw, offers the throw/heavy decision.
    public class OpponentBrain
    {
        public enum Pattern { Pressure, Defensive }
        public Pattern pattern = Pattern.Pressure;
        double nextDecision;
        int phraseStep;

        public Intent Think(CombatWorld world, FighterSim me, FighterSim foe)
        {
            var intent = new Intent();
            if (me.Defeated || foe.Defeated) return intent;
            var to = foe.Position - me.Position;
            to.y = 0;
            float dist = to.magnitude;
            var dir = dist > 1e-4f ? to / dist : Vector3.forward;
            me.LockTarget = foe.Id;

            bool foeCasting = foe.State.Kind == ActionKind.CastPlain && foe.State.Phase == ActionPhase.Startup || foe.State.Kind == ActionKind.CastHold;
            bool foeReaching = foe.State.Kind == ActionKind.Throw && foe.State.Phase == ActionPhase.Startup;
            bool foeStriking = (foe.State.Kind == ActionKind.LightL || foe.State.Kind == ActionKind.LightR || foe.State.Kind == ActionKind.Heavy) && foe.State.Phase == ActionPhase.Startup;

            if (pattern == Pattern.Defensive)
            {
                intent.GuardHeld = dist < 2.5f && me.Guard > 20f;
                if (me.LastKind == ActionKind.Throw && dist < 2f) intent.Move = new Vector2(-dir.x, -dir.z);   // back off after a failed throw
                else if (dist > 1.6f) intent.Move = new Vector2(dir.x, dir.z) * 0.6f;
                if (foeCasting && dist < 1.1f) { intent.GuardHeld = false; intent.Light = true; }
                return intent;
            }

            // Pressure
            if ((foeCasting || foeReaching) && dist < 1.15f) { intent.Light = true; return intent; }
            if (foeStriking && dist < 1.3f && phraseStep % 2 == 1) { intent.GuardHeld = true; return intent; }
            if (dist > 1.05f) { intent.Move = new Vector2(dir.x, dir.z); intent.Run = dist > 3f; return intent; }
            if (world.Time < nextDecision) return intent;
            if (!me.Free) return intent;
            // two-hit attempt, then a visible pause before the next one
            intent.Light = true;
            phraseStep++;
            nextDecision = world.Time + (phraseStep % 2 == 0 ? 1.6 : 0.5);
            return intent;
        }
    }
}
