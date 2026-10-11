using System.Collections.Generic;
using Guildbound.Progress;
using Guildbound.Rules;

namespace Guildbound.Play
{
    // IInventoryContext over the live game: the objective and keepsakes come from GuildState (single source of
    // truth), combat state from the sparring fight.
    public class PlayInventoryContext : IInventoryContext
    {
        readonly GuildState guild;
        readonly System.Func<bool> inCombat;
        readonly List<DeliveredContract> delivered = new List<DeliveredContract>();

        public PlayInventoryContext(GuildState guild, System.Func<bool> inCombat)
        {
            this.guild = guild;
            this.inCombat = inCombat;
        }

        public string CarriedObjective => guild.Carried;
        public bool InCombat => inCombat();

        public IReadOnlyList<DeliveredContract> Delivered
        {
            get
            {
                delivered.Clear();
                foreach (var c in guild.Contracts)
                    if (guild.Paid.Contains(c.Id))   // keepsake stays while the contract is replayed as practice
                        delivered.Add(new DeliveredContract { Id = c.Id, Title = c.Title });
                return delivered;
            }
        }
    }
}
