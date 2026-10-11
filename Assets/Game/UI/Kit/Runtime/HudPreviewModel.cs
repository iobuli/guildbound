using System.Collections.Generic;
using Guildbound.Contracts;
using UnityEngine;

namespace Guildbound.UI
{
    // Stand-in IHudModel for the HUD preview scene: inspector values, or a demo cycle through every state.
    public class HudPreviewModel : MonoBehaviour, IHudModel
    {
        public bool demo = true;
        [Range(0, 1)] public float hp = 0.8f;
        public int guardCurrent = 4, guardMax = 5;
        public bool guardBroken;
        public FireState fire = FireState.Ready;
        [Range(0, 1)] public float charge, cooldown;
        public string carried = "Sealed guild marker", returnInstruction = "Return the seal";
        public string prompt = "Open the guild board", promptKey = "E";
        [Range(-1, 1)] public float recover = -1f;
        public int wallet = 30;
        public string trust = "Established";
        [Range(0, 1)] public float trustProgress = 0.62f;
        public List<ContractCard> contracts = new()
        {
            new ContractCard { Id = "c1", Title = "Recover the sealed guild marker.", Objective = "Bring the seal back to the hall.", Service = "Restores the gate route.", Available = true },
            new ContractCard { Id = "c2", Title = "Recover the guild parcel.", Objective = "Carry the parcel home unbroken.", Service = "Activates the preparation stall.", Available = true },
        };
        readonly Queue<HudAlert> alerts = new();
        float phase;

        public float Hp01 => hp;
        public int GuardCurrent => guardCurrent;
        public int GuardMax => guardMax;
        public bool GuardBroken => guardBroken;
        public FireState Fire => fire;
        public float FireCharge01 => charge;
        public float FireCooldown01 => cooldown;
        public string CarriedObjective => carried;
        public string ReturnInstruction => returnInstruction;
        public string Prompt => prompt;
        public string PromptKey => promptKey;
        public bool InCombat => fire != FireState.Locked;
        public float Recover01 => recover;
        public int Wallet => wallet;
        public string TrustName => trust;
        public float TrustProgress01 => trustProgress;
        public int ContractCount => contracts.Count;
        public ContractCard GetContract(int index) => contracts[index];
        public bool TryDequeueAlert(out HudAlert alert) => alerts.TryDequeue(out alert);

        public void RequestAcceptContract(string id)
        {
            for (int i = 0; i < contracts.Count; i++)
            {
                if (contracts[i].Id != id) continue;
                var c = contracts[i]; c.Accepted = true; contracts[i] = c;
                alerts.Enqueue(new HudAlert { Text = "Contract accepted: " + c.Title, Level = AlertLevel.Info });
            }
        }

        public void Push(string text, AlertLevel level) => alerts.Enqueue(new HudAlert { Text = text, Level = level });

        void Update()
        {
            if (!demo) return;
            // 12 s loop: ready -> charge -> release -> cooldown -> interrupted; HP dips, guard breaks once.
            float prev = phase;
            phase = Time.time % 12f;
            if (phase < 2f) { fire = FireState.Ready; charge = 0; cooldown = 0; }
            else if (phase < 4f) { fire = FireState.Charging; charge = (phase - 2f) / 2f; }
            else if (phase < 4.4f) fire = FireState.Release;
            else if (phase < 8f) { fire = FireState.Cooldown; cooldown = 1f - (phase - 4.4f) / 3.6f; }
            else if (phase < 9f) fire = FireState.Interrupted;
            else fire = FireState.Ready;
            hp = 0.2f + 0.75f * (0.5f + 0.5f * Mathf.Cos(phase / 12f * Mathf.PI * 2f));
            guardCurrent = Mathf.Clamp(5 - (int)(phase / 2f), 0, 5);
            guardBroken = phase > 10f && phase < 11.5f;
            recover = phase > 9.5f && phase < 11.5f ? (phase - 9.5f) / 2f : -1f;
            if (prev < 9f && phase >= 9f) Push("Charge interrupted: hit while casting", AlertLevel.Warning);
            if (prev < 10f && phase >= 10f) Push("Guard broken", AlertLevel.Critical);
        }
    }
}
