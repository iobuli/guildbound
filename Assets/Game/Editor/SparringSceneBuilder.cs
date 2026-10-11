using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;

namespace Guildbound.Editor
{
    /// <summary>
    /// Builds Assets/Game/Scenes/Sparring.unity - the 12 x 12 m combat test floor - from the Low Poly
    /// Tarvern Pack kit. Reproducible and headless-capable:
    ///
    ///   Menu:     Guildbound/Scenes/Build Sparring
    ///   CLI:      Unity.exe -batchmode -quit -projectPath &lt;proj&gt; -executeMethod Guildbound.Editor.SparringSceneBuilder.Build
    ///
    /// Geometry is measured from the source prefabs, never assumed:
    ///   Floor Tile.prefab    corner pivot, mesh spans x[-4,0] y[0,0.2] z[0,4] from the pivot,
    ///                        so the root sits at y = -0.2 to put the walkable surface at y = 0.
    ///   Straight Wall.prefab corner pivot, mesh spans x[0,4] y[0,6] z[0,0.4] from the pivot,
    ///                        so Y scale 0.25 gives a 1.5 m wall.
    ///
    /// Because the pivots are on a corner and the floor mesh runs to -X but +Z, a naive centred
    /// {-2,0,2} grid leaves a quadrant of the arena empty; the offsets below are what actually tile.
    /// Walls sit at y = 0 (the floor surface) so they are not half-buried.
    ///
    /// This scene is geometry, light and spawn points only. The gameplay components - SparringDriver,
    /// CombatHudModel, SparringCamera and the Hud.prefab instance - are added by the Play-layer owner.
    /// </summary>
    public static class SparringSceneBuilder
    {
        public const string ScenePath = "Assets/Game/Scenes/Sparring.unity";

        const string FloorPrefabPath = "Assets/Low Poly Tarvern Pack/Prefabs/Building/Floor Tile.prefab";
        const string WallPrefabPath = "Assets/Low Poly Tarvern Pack/Prefabs/Building/Straight Wall.prefab";

        // Floor mesh spans y[0, 0.2] above its pivot, so -0.2 puts the walkable surface at y = 0.
        const float FloorY = -0.2f;
        const float WallY = 0f;
        const float WallScaleY = 0.25f;   // 6 m source height -> 1.5 m

        const float ArenaHalf = 6f;       // arena spans -6..6 on both axes
        const float CameraAimY = 1.2f;

        [MenuItem("Guildbound/Scenes/Build Sparring")]
        public static void BuildMenu() => Build();

        public static string Build()
        {
            if (EditorApplication.isPlaying)
                throw new InvalidOperationException("Stop Play mode before building Sparring.unity.");

            var open = EditorSceneManager.GetActiveScene();
            if (open.isDirty)
                EditorSceneManager.SaveOpenScenes();   // preserve, never discard, someone else's work

            var floorPrefab = LoadPrefab(FloorPrefabPath);
            var wallPrefab = LoadPrefab(WallPrefabPath);

            var scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
            var root = new GameObject("Arena");

            int floorCount = BuildFloor(root.transform, floorPrefab);
            int wallCount = BuildWalls(root.transform, wallPrefab);
            BuildCamera();
            BuildLight();
            BuildSpawn("SpawnA", new Vector3(-1.5f, 0f, 0f), 90f);    // faces +X, toward SpawnB
            BuildSpawn("SpawnB", new Vector3(1.5f, 0f, 0f), -90f);    // faces -X, toward SpawnA
            BuildEventSystem();

            Directory.CreateDirectory(Path.GetDirectoryName(ScenePath) ?? "Assets/Game/Scenes");

            EditorSceneManager.MarkSceneDirty(scene);
            if (!EditorSceneManager.SaveScene(scene, ScenePath))
                throw new IOException("Failed to save " + ScenePath);

            RegisterInBuildSettings();
            AssetDatabase.SaveAssets();
            AssetDatabase.Refresh();

            var sb = new StringBuilder();
            sb.Append("Built ").Append(ScenePath).Append(" - ");
            sb.Append(floorCount).Append(" floor tiles, ").Append(wallCount).Append(" wall pieces.");
            sb.Append(" Arena ").Append(ArenaHalf * 2f).Append(" x ").Append(ArenaHalf * 2f).Append(" m, floor top at y = 0, walls 1.5 m.");
            sb.Append(" Camera at (0, 2.6, -7.5) aiming at (0, ").Append(CameraAimY).Append(", 0).");
            sb.Append(" SpawnA (-1.5,0,0) facing +X, SpawnB (1.5,0,0) facing -X.");
            sb.Append(" Scene left open. Gameplay components and the Hud instance are still to be added.");
            return sb.ToString();
        }

        // ---- arena ----

        static int BuildFloor(Transform parent, GameObject prefab)
        {
            var floorRoot = new GameObject("Floor");
            floorRoot.transform.SetParent(parent, false);

            // Floor mesh occupies x[-4,0] z[0,4] from the pivot, so these offsets tile -6..6 exactly.
            var offsetsX = new[] { 6f, 2f, -2f };
            var offsetsZ = new[] { -6f, -2f, 2f };

            int n = 0;
            foreach (var x in offsetsX)
            {
                foreach (var z in offsetsZ)
                {
                    var tile = Instantiate(prefab, floorRoot.transform);
                    tile.name = string.Format("Floor Tile ({0},{1})", x, z);
                    tile.transform.localPosition = new Vector3(x, FloorY, z);
                    tile.transform.localRotation = Quaternion.identity;
                    tile.transform.localScale = Vector3.one;
                    MarkStatic(tile);
                    n++;
                }
            }
            return n;
        }

        static int BuildWalls(Transform parent, GameObject prefab)
        {
            var wallRoot = new GameObject("Walls");
            wallRoot.transform.SetParent(parent, false);

            // Wall mesh occupies x[0,4] z[0,0.4] from the pivot, so each piece is placed by the corner
            // its mesh grows into. 3 pieces per side, 4 sides, exactly closing the 12 x 12 arena.
            // x = world X, y = yaw in degrees, z = world Z.
            var pieces = new[]
            {
                // North (+Z): mesh grows +X, +Z
                new Vector3(-6f,    0f,  6f),
                new Vector3(-2f,    0f,  6f),
                new Vector3( 2f,    0f,  6f),
                // South (-Z): yaw 180, mesh grows -X, -Z
                new Vector3( 6f,  180f, -6f),
                new Vector3( 2f,  180f, -6f),
                new Vector3(-2f,  180f, -6f),
                // East (+X): yaw 90, mesh grows +X, -Z
                new Vector3( 6f,   90f,  6f),
                new Vector3( 6f,   90f,  2f),
                new Vector3( 6f,   90f, -2f),
                // West (-X): yaw 270, mesh grows -X, +Z
                new Vector3(-6f,  270f, -6f),
                new Vector3(-6f,  270f, -2f),
                new Vector3(-6f,  270f,  2f),
            };

            int n = 0;
            foreach (var p in pieces)
            {
                var wall = Instantiate(prefab, wallRoot.transform);
                wall.name = string.Format("Wall ({0:0},{1:0}) {2:0}", p.x, p.z, p.y);
                wall.transform.localPosition = new Vector3(p.x, WallY, p.z);
                wall.transform.localRotation = Quaternion.Euler(0f, p.y, 0f);
                wall.transform.localScale = new Vector3(1f, WallScaleY, 1f);
                MarkStatic(wall);
                n++;
            }
            return n;
        }

        // ---- scene furniture ----

        static void BuildCamera()
        {
            var go = new GameObject("Main Camera") { tag = "MainCamera" };
            go.transform.position = new Vector3(0f, 2.6f, -7.5f);
            go.transform.LookAt(new Vector3(0f, CameraAimY, 0f));

            var cam = go.AddComponent<Camera>();
            cam.fieldOfView = 60f;
            cam.nearClipPlane = 0.1f;
            cam.farClipPlane = 200f;
            go.AddComponent<AudioListener>();
        }

        static void BuildLight()
        {
            var go = new GameObject("Directional Light");
            go.transform.rotation = Quaternion.Euler(50f, -30f, 0f);

            var light = go.AddComponent<Light>();
            light.type = LightType.Directional;
            light.color = new Color(1f, 0.9569f, 0.8392f);
            light.intensity = 1.1f;
            light.shadows = LightShadows.Soft;
        }

        static void BuildSpawn(string name, Vector3 localPosition, float yaw)
        {
            var go = new GameObject(name);
            go.transform.localPosition = localPosition;
            go.transform.localRotation = Quaternion.Euler(0f, yaw, 0f);
        }

        static void BuildEventSystem()
        {
            var go = new GameObject("EventSystem");
            go.AddComponent<UnityEngine.EventSystems.EventSystem>();

            // Resolve by name so a missing Input System module can never break compilation.
            var module = Type.GetType("UnityEngine.InputSystem.UI.InputSystemUIInputModule, Unity.InputSystem");
            if (module != null)
                go.AddComponent(module);
            else
                Debug.LogWarning("SparringSceneBuilder: InputSystemUIInputModule not found; EventSystem has no input module.");
        }

        // ---- helpers ----

        static GameObject Instantiate(GameObject prefab, Transform parent)
        {
            var go = (GameObject)PrefabUtility.InstantiatePrefab(prefab);
            go.transform.SetParent(parent, false);
            return go;
        }

        static GameObject LoadPrefab(string path)
        {
            var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(path);
            if (prefab == null)
                throw new FileNotFoundException("Required kit prefab not found: " + path);
            return prefab;
        }

        static void MarkStatic(GameObject go)
        {
            GameObjectUtility.SetStaticEditorFlags(go,
                StaticEditorFlags.ContributeGI | StaticEditorFlags.BatchingStatic);
        }

        static void RegisterInBuildSettings()
        {
            var scenes = new List<EditorBuildSettingsScene>(EditorBuildSettings.scenes);
            if (scenes.Exists(s => s.path == ScenePath))
                return;
            scenes.Add(new EditorBuildSettingsScene(ScenePath, true));
            EditorBuildSettings.scenes = scenes.ToArray();
        }
    }
}
