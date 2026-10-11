using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text;
using UnityEditor;
using UnityEngine;

namespace Guildbound.Editor
{
    // Pulls owned Asset Store packages through the Package Manager's own internal Asset Store services.
    // List and Download need a normal editor signed in through the Hub (not batch mode); Import runs in batch.
    public static class Fetch
    {
        const BindingFlags All = BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.Static;
        const string Internal = "UnityEditor.PackageManager.UI.Internal.";
        static readonly StringBuilder report = new StringBuilder();
        static double deadline;
        static bool listed;

        static IEnumerable<Type> Types() => AppDomain.CurrentDomain.GetAssemblies()
            .SelectMany(a => { try { return a.GetTypes(); } catch (ReflectionTypeLoadException e) { return e.Types.Where(t => t != null); } });
        static Type Find(string name) => Types().First(t => t.FullName == Internal + name);

        static object Service(string contract)
        {
            var container = Find("ServicesContainer");
            var instance = container.GetProperty("instance", All | BindingFlags.FlattenHierarchy)?.GetValue(null)
                ?? typeof(ScriptableSingleton<>).MakeGenericType(container).GetProperty("instance", All).GetValue(null);
            return container.GetMethod("Resolve", All).MakeGenericMethod(Find(contract)).Invoke(instance, null);
        }

        static void Subscribe(object target, string eventName, string handler)
        {
            var info = target.GetType().GetEvent(eventName, All) ?? target.GetType().GetInterfaces().Select(i => i.GetEvent(eventName, All)).First(e => e != null);
            info.AddEventHandler(target, Delegate.CreateDelegate(info.EventHandlerType, typeof(Fetch).GetMethod(handler, All)));
        }

        static object Field(object target, string name) => target.GetType().GetField(name, All).GetValue(target);

        // Internal argument types have no default constructor: call the shortest one, filling known names.
        static object Construct(Type type, Dictionary<string, object> values)
        {
            var ctor = type.GetConstructors(All).OrderBy(c => c.GetParameters().Length).First();
            report.AppendLine("ctor " + type.Name + "(" + string.Join(", ", ctor.GetParameters().Select(p => p.ParameterType.Name + " " + p.Name)) + ")");
            var args = ctor.GetParameters().Select(p => values.TryGetValue(p.Name, out var v) ? Convert.ChangeType(v, p.ParameterType)
                : p.HasDefaultValue ? p.DefaultValue
                : p.ParameterType.IsValueType ? Activator.CreateInstance(p.ParameterType)
                : p.ParameterType == typeof(string) ? ""
                : p.ParameterType.IsGenericType && p.ParameterType.GetGenericTypeDefinition() == typeof(List<>) ? Activator.CreateInstance(p.ParameterType) : null).ToArray();
            var made = ctor.Invoke(args);
            foreach (var pair in values) type.GetField(pair.Key, All)?.SetValue(made, Convert.ChangeType(pair.Value, type.GetField(pair.Key, All).FieldType));
            return made;
        }

        static void Fail(string file, Exception error)
        {
            Directory.CreateDirectory("Logs");
            File.WriteAllText(file, "ERROR " + error + "\n" + report);
            Debug.Log("GUILDBOUND_FETCH_FAILED " + error.Message);
            Exit(1);
        }
        static void Exit(int code) { if (Array.IndexOf(Environment.GetCommandLineArgs(), "-guildboundExit") >= 0) EditorApplication.Exit(code); }

        // Pass 1: every owned package with its purchase date, plus what is already downloaded locally.
        [MenuItem("Guildbound/Fetch/List Owned Packages")]
        // The Asset Store services only come up with the Package Manager window, so open it and give it time.
        public static void List()
        {
            UnityEditor.PackageManager.UI.Window.Open("");
            deadline = EditorApplication.timeSinceStartup + 10;
            EditorApplication.update += AfterWindow;
        }

        static void AfterWindow()
        {
            if (EditorApplication.timeSinceStartup < deadline) return;
            EditorApplication.update -= AfterWindow;
            try
            {
                var connect = Types().First(t => t.FullName == "UnityEditor.Connect.UnityConnect");
                var instance = connect.GetProperty("instance", All).GetValue(null);
                report.AppendLine("loggedIn=" + connect.GetProperty("loggedIn", All)?.GetValue(instance));
                var client = Service("IAssetStoreClient");
                Subscribe(client, "onProductListFetched", nameof(OnPurchases));
                var args = Construct(Find("PurchasesQueryArgs"), new Dictionary<string, object> { ["startIndex"] = 0, ["limit"] = 1000L });
                client.GetType().GetMethod("ListPurchases", All).Invoke(client, new[] { args });
                deadline = EditorApplication.timeSinceStartup + 180;
                EditorApplication.update += WaitForList;
            }
            catch (Exception error) { Fail("Logs/fetch-purchases.txt", error); }
        }

        static void OnPurchases(object purchases)
        {
            report.AppendLine("total=" + Field(purchases, "total"));
            foreach (var item in (IEnumerable)Field(purchases, "list"))
                report.AppendLine(Field(item, "productId") + "\t" + Field(item, "purchasedTime") + "\t" + Field(item, "displayName") + "\t" + string.Join(",", (string[])Field(item, "tags") ?? new string[0]));
            listed = true;
        }

        static void WaitForList()
        {
            if (!listed && EditorApplication.timeSinceStartup < deadline) return;
            EditorApplication.update -= WaitForList;
            try { ListLocal(); } catch (Exception error) { Fail("Logs/fetch-purchases.txt", error); }
        }

        static void ListLocal()
        {
            var cache = Service("IAssetStoreCache");
            report.AppendLine("== downloaded locally");
            foreach (var local in (IEnumerable)cache.GetType().GetProperty("localInfos", All).GetValue(cache))
                report.AppendLine(Field(local, "productId") + "\t" + Field(local, "title") + "\t" + Field(local, "packagePath"));
            Directory.CreateDirectory("Logs");
            File.WriteAllText("Logs/fetch-purchases.txt", (listed ? "" : "TIMEOUT: purchase list never arrived\n") + report);
            Debug.Log("GUILDBOUND_FETCH_LISTED " + listed);
            Exit(listed ? 0 : 1);
        }

        // Pass 2: download the product ids in Logs/fetch-ids.txt into Unity's Asset Store cache.
        [MenuItem("Guildbound/Fetch/Download Chosen Packages")]
        public static void Download()
        {
            UnityEditor.PackageManager.UI.Window.Open("");
            deadline = EditorApplication.timeSinceStartup + 10;
            EditorApplication.update += BeforeDownload;
        }

        static void BeforeDownload()
        {
            if (EditorApplication.timeSinceStartup < deadline) return;
            EditorApplication.update -= BeforeDownload;
            try { StartDownload(); } catch (Exception error) { Fail("Logs/fetch-downloaded.txt", error); }
        }

        static void StartDownload()
        {
            var ids = File.ReadAllLines("Logs/fetch-ids.txt").Where(l => l.Trim().Length > 0).Select(l => long.Parse(l.Split('\t')[0])).ToList();
            var manager = Service("IAssetStoreDownloadManager");
            var download = manager.GetType().GetMethods(All).First(m => m.Name == "Download" && m.GetParameters().Length == 1 && m.GetParameters()[0].ParameterType == typeof(IEnumerable<long>));
            bool ok = (bool)download.Invoke(manager, new object[] { ids });
            Debug.Log("GUILDBOUND_FETCH_DOWNLOAD_STARTED " + ok + " " + ids.Count);
            wanted = ids; started = EditorApplication.timeSinceStartup;
            deadline = started + 1800;
            EditorApplication.update += WaitForDownloads;
        }

        static List<long> wanted;
        static double started;

        // Every product must reach a final state (Completed, Error or Aborted); its operation records the path or the error.
        static void WaitForDownloads()
        {
            var manager = Service("IAssetStoreDownloadManager");
            var get = manager.GetType().GetMethod("GetDownloadOperation", All);
            var ops = wanted.Select(id => (id, op: get.Invoke(manager, new object[] { id }))).ToList();
            string State(object op) => op.GetType().GetProperty("state", All).GetValue(op).ToString();
            bool done = EditorApplication.timeSinceStartup - started > 20 && ops.All(o => o.op != null && new[] { "Completed", "Error", "Aborted" }.Any(f => State(o.op).Contains(f)));
            if (!done && EditorApplication.timeSinceStartup < deadline) return;
            EditorApplication.update -= WaitForDownloads;
            var lines = ops.Select(o => o.id + "\t" + (o.op == null ? "no operation\t\t" : State(o.op) + "\t" + o.op.GetType().GetProperty("packageNewPath", All).GetValue(o.op) + "\t" + o.op.GetType().GetProperty("errorMessage", All).GetValue(o.op))).ToList();
            File.WriteAllLines("Logs/fetch-downloaded.txt", lines);
            Debug.Log("GUILDBOUND_FETCH_DOWNLOADED " + lines.Count(l => l.Contains("Completed")) + "/" + lines.Count + (done ? "" : " (timed out)"));
            Exit(done ? 0 : 1);
        }
    }
}
