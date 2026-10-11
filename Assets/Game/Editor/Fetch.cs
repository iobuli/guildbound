using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text;
using UnityEditor;

namespace Guildbound.Editor
{
    // Pulls owned Asset Store packages into the project through the Package Manager's own internal
    // Asset Store services. Needs a normal editor signed in through the Hub, not batch mode.
    public static class Fetch
    {
        const BindingFlags All = BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.Static;

        static IEnumerable<Type> Types() => AppDomain.CurrentDomain.GetAssemblies()
            .SelectMany(a => { try { return a.GetTypes(); } catch (ReflectionTypeLoadException e) { return e.Types.Where(t => t != null); } });

        // Writes the live signatures of the Package Manager's Asset Store services to Logs/fetch-api.txt.
        [MenuItem("Guildbound/Fetch/Describe Asset Store API")]
        public static void Describe()
        {
            var text = new StringBuilder();
            foreach (var type in Types().Where(t => t.Namespace == "UnityEditor.PackageManager.UI.Internal" && (t.Name.Contains("AssetStore") || t.Name == "ServicesContainer" || t.Name.Contains("Purchase") || t.Name.Contains("Download"))))
            {
                text.AppendLine("== " + type.FullName + (type.IsInterface ? " (interface)" : ""));
                foreach (var m in type.GetMethods(All | BindingFlags.DeclaredOnly))
                    text.AppendLine("  " + (m.IsStatic ? "static " : "") + m.ReturnType.Name + " " + m.Name + (m.IsGenericMethodDefinition ? "<T>" : "") + "(" + string.Join(", ", m.GetParameters().Select(p => p.ParameterType.Name + " " + p.Name)) + ")");
                foreach (var p in type.GetProperties(All | BindingFlags.DeclaredOnly)) text.AppendLine("  prop " + p.PropertyType.Name + " " + p.Name);
                foreach (var f in type.GetFields(All | BindingFlags.DeclaredOnly)) text.AppendLine("  field " + f.FieldType.Name + " " + f.Name);
                foreach (var e in type.GetEvents(All | BindingFlags.DeclaredOnly)) text.AppendLine("  event " + e.EventHandlerType + " " + e.Name);
            }
            Directory.CreateDirectory("Logs");
            File.WriteAllText("Logs/fetch-api.txt", text.ToString());
            UnityEngine.Debug.Log("GUILDBOUND_FETCH_DESCRIBED " + text.Length);
            if (Array.IndexOf(Environment.GetCommandLineArgs(), "-guildboundExit") >= 0) EditorApplication.Exit(0);
        }
    }
}
