namespace Guildbound.Contracts
{
    public enum FireState { Locked, Ready, Charging, Release, Cooldown, Interrupted }
    public enum AlertLevel { Info, Warning, Critical }

    public struct HudAlert
    {
        public string Text;
        public AlertLevel Level;
    }

    public enum MapBlipKind { Self, Opponent, Objective, Door, ServicePoint }

    // A point of interest in WORLD space (metres on the ground plane). The HUD owns the projection onto the map.
    public struct MapBlip
    {
        public float X, Z;
        public MapBlipKind Kind;
    }

    public struct ContractCard
    {
        public string Id, Title, Objective, Service;
        public bool Accepted, Available;
    }

    // Everything the HUD shows, read every frame. The HUD never owns or changes game state; it only calls the
    // request methods, which the rules code may refuse.
    public interface IHudModel
    {
        float Hp01 { get; }
        int GuardCurrent { get; }
        int GuardMax { get; }
        bool GuardBroken { get; }

        FireState Fire { get; }
        float FireCharge01 { get; }
        float FireCooldown01 { get; }   // 1 = just fired, 0 = ready

        string CarriedObjective { get; }     // null or empty when the slot is empty
        string ReturnInstruction { get; }    // e.g. "Return the seal"; null when not relevant
        string Prompt { get; }               // context prompt text, null when none
        string PromptKey { get; }

        bool InCombat { get; }
        float Recover01 { get; }             // < 0 when not recovering

        int Wallet { get; }
        string TrustName { get; }
        float TrustProgress01 { get; }
        int ContractCount { get; }
        ContractCard GetContract(int index);

        bool TryDequeueAlert(out HudAlert alert);
        void RequestAcceptContract(string id);

        bool HasMapData { get; }                 // false: the HUD shows "no map data", never an invented map
        float CompassHeadingDegrees { get; }     // view direction: 0 = +Z, clockwise, 0..360
        void GetMapBounds(out float minX, out float minZ, out float maxX, out float maxZ);   // world space
        int MapBlipCount { get; }
        MapBlip GetMapBlip(int index);
    }
}
