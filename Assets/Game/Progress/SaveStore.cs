using System;
using System.IO;
using UnityEngine;

namespace Guildbound.Progress
{
    // Complete snapshot plus a recoverable previous file. Write order: temp -> rotate current to .prev -> move temp
    // to current, so an interrupted write never leaves the player without a loadable save.
    public class SaveStore
    {
        readonly string dir;
        public string Current => Path.Combine(dir, "guild.json");
        public string Previous => Path.Combine(dir, "guild.prev.json");
        string Temp => Path.Combine(dir, "guild.tmp");

        public SaveStore(string directory)
        {
            dir = directory;
            Directory.CreateDirectory(dir);
        }

        public void Write(GuildState.Snapshot snapshot)
        {
            File.WriteAllText(Temp, JsonUtility.ToJson(snapshot));
            if (File.Exists(Current))
            {
                if (File.Exists(Previous)) File.Delete(Previous);
                File.Move(Current, Previous);
            }
            File.Move(Temp, Current);
        }

        // Current if valid, else the previous file, else null (fresh game).
        public GuildState.Snapshot Read()
        {
            return TryRead(Current) ?? TryRead(Previous);
        }

        static GuildState.Snapshot TryRead(string path)
        {
            if (!File.Exists(path)) return null;
            try
            {
                var s = JsonUtility.FromJson<GuildState.Snapshot>(File.ReadAllText(path));
                return s != null && s.version >= 1 ? s : null;
            }
            catch (ArgumentException) { return null; }
        }
    }
}
