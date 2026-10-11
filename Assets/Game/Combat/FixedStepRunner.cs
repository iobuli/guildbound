using System;
using System.Collections.Generic;

namespace Guildbound.Combat
{
    // Drives a CombatWorld from variable render frames. Ticks are counted from total elapsed time (not an
    // accumulating float), so 30/60/120 fps produce the same tick count. A press seen in a frame is latched until
    // exactly one tick consumes it; held inputs and movement use the latest frame's values.
    public class FixedStepRunner
    {
        public readonly CombatWorld World;
        public int MaxTicksPerFrame = 30;
        double elapsed;
        long ticks;
        readonly List<Intent> latched = new List<Intent>();

        public FixedStepRunner(CombatWorld world) => World = world;

        public int Frame(float dt, IReadOnlyList<Intent> frameIntents)
        {
            elapsed += dt;
            while (latched.Count < World.Fighters.Count) latched.Add(default);
            for (int i = 0; i < latched.Count; i++)
            {
                var input = frameIntents != null && i < frameIntents.Count ? frameIntents[i] : default;
                var l = latched[i];
                var merged = input;
                merged.Light |= l.Light; merged.Heavy |= l.Heavy; merged.Step |= l.Step;
                merged.Throw |= l.Throw; merged.Cast |= l.Cast; merged.Recover |= l.Recover;
                latched[i] = merged;
            }
            long target = (long)Math.Floor(elapsed / Tuning.Step + 1e-6);
            int ran = 0;
            while (ticks < target && ran < MaxTicksPerFrame)
            {
                World.Tick(latched);
                for (int i = 0; i < latched.Count; i++) latched[i] = latched[i].WithoutPresses();
                ticks++;
                ran++;
            }
            if (ticks < target) ticks = target;   // spiral guard: drop excess time instead of freezing the frame
            return ran;
        }
    }
}
