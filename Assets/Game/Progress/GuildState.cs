using System;
using System.Collections.Generic;

namespace Guildbound.Progress
{
    public enum AttemptState { Available, Accepted, Carried, Delivered }

    public class ContractDef
    {
        public string Id, Title, Objective, ObjectiveItem, Route, Service;
        public int Payment, Trust;
    }

    // Contract/attempt state, wallet, trust, services and attunement (GDD v5.1 sections 5-7). Pure rules: delivery
    // commits payment, service and access together exactly once; replays pay nothing; defeat/abandon lose only the
    // current objective; nothing needs coin or an item to finish.
    public class GuildState
    {
        public static readonly string[] TrustBands = { "Recruit", "Established", "Trusted", "Sworn" };

        public readonly List<ContractDef> Contracts = new List<ContractDef>();
        public readonly Dictionary<string, AttemptState> Attempts = new Dictionary<string, AttemptState>();
        public readonly HashSet<string> Paid = new HashSet<string>();
        public readonly HashSet<string> Services = new HashSet<string>();
        public int Wallet, TrustPoints;
        public bool Attuned;
        public string Active;            // accepted or carried contract, null when none
        public string Carried;           // objective item in the single expedition slot, null when empty

        public event Action Changed;

        public string TrustName => TrustBands[Math.Min(TrustPoints / 2, TrustBands.Length - 1)];
        public float TrustProgress01 => TrustPoints >= 2 * (TrustBands.Length - 1) ? 1f : TrustPoints % 2 / 2f;
        public bool AttunementOffered => Paid.Count >= 1 && !Attuned;

        public static GuildState Prototype()
        {
            var g = new GuildState();
            g.Contracts.Add(new ContractDef { Id = "c1", Title = "Recover the tool crate.", Objective = "Bring the crate back to the hall.", ObjectiveItem = "Tool crate", Route = "Exposed timber road", Service = "Repairs the gate: a shorter return.", Payment = 30, Trust = 1 });
            g.Contracts.Add(new ContractDef { Id = "c2", Title = "Recover the supplies.", Objective = "Carry the supplies home.", ObjectiveItem = "Supplies", Route = "Sheltered path", Service = "Opens the preparation stall.", Payment = 30, Trust = 1 });
            foreach (var c in g.Contracts) g.Attempts[c.Id] = AttemptState.Available;
            return g;
        }

        public ContractDef Find(string id) => Contracts.Find(c => c.Id == id);

        public bool Accept(string id, out string reason)
        {
            reason = null;
            var c = Find(id);
            if (c == null) { reason = "Unknown contract"; return false; }
            if (Active != null) { reason = "Finish or abandon the current contract first"; return false; }
            if (Attempts[id] == AttemptState.Delivered) { reason = null; }   // completed contracts stay free practice
            Attempts[id] = AttemptState.Accepted;
            Active = id;
            Changed?.Invoke();
            return true;
        }

        public bool PickUp(string item, out string reason)
        {
            reason = null;
            if (Carried != null) { reason = "The expedition slot is full"; return false; }
            var c = Active != null ? Find(Active) : null;
            if (c == null || c.ObjectiveItem != item || Attempts[c.Id] != AttemptState.Accepted) { reason = "Not part of an accepted contract"; return false; }
            Carried = item;
            Attempts[c.Id] = AttemptState.Carried;
            Changed?.Invoke();
            return true;
        }

        // One event: payment (first time only), service, trust, new option. Returns false if nothing to deliver.
        public bool Deliver(out string reason)
        {
            reason = null;
            var c = Active != null ? Find(Active) : null;
            if (c == null || Attempts[c.Id] != AttemptState.Carried || Carried != c.ObjectiveItem) { reason = "Nothing to deliver"; return false; }
            if (Paid.Add(c.Id))
            {
                Wallet += c.Payment;
                TrustPoints += c.Trust;
                Services.Add(c.Id);
            }
            Attempts[c.Id] = AttemptState.Delivered;
            Carried = null;
            Active = null;
            Changed?.Invoke();
            return true;
        }

        // Defeat or abandon: back to the bench, only the current objective is lost.
        public void ResetAttempt()
        {
            if (Active != null && Attempts[Active] != AttemptState.Delivered)
                Attempts[Active] = Paid.Contains(Active) ? AttemptState.Delivered : AttemptState.Available;
            Active = null;
            Carried = null;
            Changed?.Invoke();
        }

        public bool CompleteAttunement(out string reason)
        {
            reason = null;
            if (!AttunementOffered) { reason = Attuned ? "Already attuned" : "Offered after the first delivery"; return false; }
            Attuned = true;
            Changed?.Invoke();
            return true;
        }

        // ---------------- snapshot ----------------

        [Serializable]
        public class Snapshot
        {
            public int version;          // 0 = invalid or empty file
            public int wallet, trust;
            public bool attuned;
            public List<string> paid = new List<string>();
            public List<string> services = new List<string>();
            public List<string> delivered = new List<string>();
            public int prepItems;
            public List<string> garments = new List<string>();   // owned dressing-corner pieces (AMEND-INV-01 item 4)
            public string equipped;                              // worn garment id, empty when none
        }

        // Only persistent state: never an active attempt, carried item, charge, cooldown or scene reference.
        public Snapshot ToSnapshot(int prepItems, IEnumerable<string> garments = null, string equipped = null)
        {
            var s = new Snapshot { version = 1, wallet = Wallet, trust = TrustPoints, attuned = Attuned, prepItems = prepItems, equipped = equipped ?? "" };
            if (garments != null) s.garments.AddRange(garments);
            s.paid.AddRange(Paid);
            s.services.AddRange(Services);
            s.delivered.AddRange(Paid);   // completed = paid once; a practice replay (Accepted/Carried) must not hide it
            s.paid.Sort(); s.services.Sort(); s.delivered.Sort();
            return s;
        }

        public void Apply(Snapshot s)
        {
            Wallet = s.wallet;
            TrustPoints = s.trust;
            Attuned = s.attuned;
            Paid.Clear(); Paid.UnionWith(s.paid);
            Services.Clear(); Services.UnionWith(s.services);
            foreach (var c in Contracts) Attempts[c.Id] = s.delivered.Contains(c.Id) ? AttemptState.Delivered : AttemptState.Available;
            Active = null;
            Carried = null;
            Changed?.Invoke();
        }
    }
}
