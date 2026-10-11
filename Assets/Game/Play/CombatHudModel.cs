using Guildbound.Contracts;
using Guildbound.Progress;
using UnityEngine;

namespace Guildbound.Play
{
    // IHudModel over the live sparring fight (player = fighter 0) and the GuildState the driver owns (board, wallet,
    // trust, carried objective). Reads only; accepting goes through GuildState, which may refuse with a reason.
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
        GuildState G => driver.Guild;

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

        public string CarriedObjective => G.Carried;

        public string ReturnInstruction
        {
            get
            {
                var c = G.Active != null && G.Carried != null ? G.Find(G.Active) : null;
                return c != null ? "Return the " + c.ObjectiveItem.ToLowerInvariant() : null;
            }
        }
        public string Prompt => P.Defeated || driver.Opponent.Defeated ? "Reset the bout" : null;
        public string PromptKey => "Enter";

        public bool InCombat => !driver.Opponent.Defeated && (driver.Opponent.Position - P.Position).magnitude < T.RecoverThreatRadius;
        public float Recover01 => P.State.Kind == ActionKind.Recover ? P.State.Elapsed / Mathf.Max(P.State.Timing.Active, 1e-3f) : -1f;

        public int Wallet => G.Wallet;
        public string TrustName => G.TrustName;
        public float TrustProgress01 => G.TrustProgress01;
        public int ContractCount => G.Contracts.Count;

        // Accepted/Available line up with GuildState.Accept: one active contract at a time; a delivered contract stays
        // available as free practice.
        public ContractCard GetContract(int index)
        {
            var c = G.Contracts[index];
            var a = G.Attempts[c.Id];
            return new ContractCard
            {
                Id = c.Id, Title = c.Title, Objective = c.Objective, Service = c.Service,
                Accepted = G.Active == c.Id,
                Available = G.Active == null && (a == AttemptState.Available || a == AttemptState.Delivered),
            };
        }

        public bool TryDequeueAlert(out HudAlert alert) => driver.Alerts.TryDequeue(out alert);

        public void RequestAcceptContract(string id)
        {
            if (!G.Accept(id, out var reason)) driver.Alerts.Enqueue(new HudAlert { Text = reason, Level = AlertLevel.Warning });
        }

        // The map is the real world geometry: the sparring BoxArena has known bounds; any other geometry has no map yet.
        public bool HasMapData => driver.World.Geometry is Combat.BoxArena;

        public float CompassHeadingDegrees
        {
            get
            {
                var f = driver.viewCamera ? driver.viewCamera.transform.forward : Vector3.forward;
                return (Mathf.Atan2(f.x, f.z) * Mathf.Rad2Deg + 360f) % 360f;
            }
        }

        public void GetMapBounds(out float minX, out float minZ, out float maxX, out float maxZ)
        {
            float h = driver.World.Geometry is Combat.BoxArena box ? box.HalfSize : 0f;
            minX = minZ = -h;
            maxX = maxZ = h;
        }

        public int MapBlipCount => driver.Opponent.Defeated ? 1 : 2;

        public MapBlip GetMapBlip(int index)
        {
            var f = index == 0 ? P : driver.Opponent;
            return new MapBlip { X = f.Position.x, Z = f.Position.z, Kind = index == 0 ? MapBlipKind.Self : MapBlipKind.Opponent };
        }
    }
}
