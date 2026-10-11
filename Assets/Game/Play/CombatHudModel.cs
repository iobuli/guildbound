using Guildbound.Contracts;
using UnityEngine;

namespace Guildbound.Play
{
    // IHudModel over the live sparring fight (player = fighter 0). Hub values are fixed until the contract system lands.
    public class CombatHudModel : MonoBehaviour, IHudModel
    {
        public SparringDriver driver;
        public UI.HudView hud;   // the inventory page binds to driver.Inventory (plain C#, not assignable in the inspector)
        const int Segments = 5;

        void Start()   // after every Awake, so the driver's InventoryModel exists
        {
            if (hud) hud.BindInventory(driver.Inventory);
        }

        Combat.FighterSim P => driver.Player;
        Combat.Tuning T => driver.World.T;

        public float Hp01 => P.Hp / P.MaxHp;
        public int GuardCurrent => Mathf.CeilToInt(P.Guard / (T.GuardMax / Segments) - 1e-3f);
        public int GuardMax => Segments;
        public bool GuardBroken => P.State.Kind == ActionKind.GuardBreak;

        public FireState Fire
        {
            get
            {
                var k = P.State.Kind;
                if (driver.World.Time - driver.LastCastInterrupt < 1.0) return FireState.Interrupted;
                if (k == ActionKind.CastHold) return FireState.Charging;
                if (k == ActionKind.CastRelease || (k == ActionKind.CastPlain && P.State.Phase != ActionPhase.Startup)) return FireState.Release;
                if (P.CastCooldown > 0) return FireState.Cooldown;
                return FireState.Ready;
            }
        }
        public float FireCharge01 => P.Charge01;
        public float FireCooldown01 => P.CastCooldown / T.CastCooldown;

        public string CarriedObjective => null;
        public string ReturnInstruction => null;
        public string Prompt => P.Defeated || driver.Opponent.Defeated ? "Reset the bout" : null;
        public string PromptKey => "Enter";

        public bool InCombat => !driver.Opponent.Defeated && (driver.Opponent.Position - P.Position).magnitude < T.RecoverThreatRadius;
        public float Recover01 => P.State.Kind == ActionKind.Recover ? P.State.Elapsed / Mathf.Max(P.State.Timing.Active, 1e-3f) : -1f;

        public int Wallet => 0;
        public string TrustName => "Recruit";
        public float TrustProgress01 => 0f;
        public int ContractCount => 0;
        public ContractCard GetContract(int index) => default;
        public bool TryDequeueAlert(out HudAlert alert) => driver.Alerts.TryDequeue(out alert);
        public void RequestAcceptContract(string id) { }
    }
}
