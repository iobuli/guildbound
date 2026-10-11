using System.IO;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;

namespace Guildbound.Editor
{
    // Builds Assets/Game/Scenes/Main.unity, the free-roam hub slot (Stage 1 of the boss work order).
    // Ground, light, camera and the palette materials only. It never saves a scene it did not create,
    // and it refuses while any open scene has unsaved edits, so no one's work is discarded.
    //
    //   Menu:  Guildbound/Scenes/Build World
    public static class WorldSceneBuilder
    {
        const string ScenePath = "Assets/Game/Scenes/Main.unity";
        const float GroundHalf = 12f;
        public const float ObjectiveX = 6f, ObjectiveZ = 6f;   // the objective marker; E2 reads its bearing from here

        [MenuItem("Guildbound/Scenes/Build World")]
        public static void Build()
        {
            for (int i = 0; i < SceneManager.sceneCount; i++)
            {
                var open = SceneManager.GetSceneAt(i);
                if (open.isDirty) throw new System.InvalidOperationException(
                    "Refusing: " + open.path + " has unsaved edits. Save or discard them first.");
            }
            if (EditorApplication.isPlaying) throw new System.InvalidOperationException("Refusing in Play mode.");

            var scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
            var palette = AssetDatabase.LoadAssetAtPath<Material>("Assets/Game/Art/Palette/QuietStone.mat");

            var ground = GameObject.CreatePrimitive(PrimitiveType.Plane);
            ground.name = "Ground";
            ground.transform.localScale = new Vector3(GroundHalf * 2f / 10f, 1f, GroundHalf * 2f / 10f);
            if (palette) ground.GetComponent<Renderer>().sharedMaterial = palette;

            var light = new GameObject("Directional Light").AddComponent<Light>();
            light.type = LightType.Directional;
            light.intensity = 1.1f;
            light.transform.rotation = Quaternion.Euler(50f, -30f, 0f);

            var cam = new GameObject("Main Camera");
            cam.tag = "MainCamera";
            cam.AddComponent<Camera>();
            cam.AddComponent<AudioListener>();
            cam.transform.position = new Vector3(0f, 2.6f, -7.5f);
            cam.transform.LookAt(new Vector3(0f, 1.2f, 0f));

            var objective = new GameObject("Objective");
            objective.transform.position = new Vector3(ObjectiveX, 0f, ObjectiveZ);

            Directory.CreateDirectory(Path.GetDirectoryName(ScenePath));
            EditorSceneManager.SaveScene(scene, ScenePath);
            Debug.Log("WorldSceneBuilder: built " + ScenePath);
        }
    }
}
