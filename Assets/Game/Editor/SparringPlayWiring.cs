using System;
using Guildbound.Play;
using Guildbound.UI;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;

namespace Guildbound.Editor
{
    // Adds the Play layer to Sparring.unity: SparringSceneBuilder makes geometry, light and spawns only.
    // Safe to rerun: components already present are reused, never duplicated.
    public static class SparringPlayWiring
    {
        const string HudPrefabPath = "Assets/Game/UI/Kit/Hud.prefab";

        [MenuItem("Guildbound/Scenes/Wire Sparring Play Layer")]
        public static void Wire()
        {
            if (EditorApplication.isPlaying) throw new InvalidOperationException("Stop Play mode first.");
            var scene = EditorSceneManager.GetActiveScene();
            if (scene.path != SparringSceneBuilder.ScenePath) throw new InvalidOperationException("Open " + SparringSceneBuilder.ScenePath + " first.");

            var cam = GameObject.Find("Main Camera").GetComponent<Camera>();
            var go = GameObject.Find("Sparring");
            if (!go) go = new GameObject("Sparring");

            var driver = Get<SparringDriver>(go);
            driver.spawnA = GameObject.Find("SpawnA").transform;
            driver.spawnB = GameObject.Find("SpawnB").transform;
            driver.viewCamera = cam;

            var model = Get<CombatHudModel>(go);
            model.driver = driver;
            Get<SparringCamera>(cam.gameObject).driver = driver;

            var view = UnityEngine.Object.FindFirstObjectByType<HudView>();
            if (!view) view = ((GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(HudPrefabPath), scene)).GetComponent<HudView>();
            view.modelSource = model;
            model.hud = view;

            EditorSceneManager.MarkSceneDirty(scene);
            EditorSceneManager.SaveScene(scene);
            Debug.Log("SparringPlayWiring: driver, HUD model, camera and Hud.prefab wired in " + scene.path);
        }

        // GetComponent can return Unity's fake null in the editor, so test with the bool operator, not ??.
        static T Get<T>(GameObject go) where T : Component
        {
            var c = go.GetComponent<T>();
            return c ? c : go.AddComponent<T>();
        }
    }
}
