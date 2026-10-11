using System;
using UnityEditor;

namespace Guildbound.Anim.Editor
{
    // Durable overrides for the measured core imports. Pack unpacking may restore its original metas.
    public sealed class CombatClipImports : AssetPostprocessor
    {
        const string Brawler = "Assets/Brawler Animations Sample/Animations/";
        const string Fighting = "Assets/FightingMotionsVolume1/FBX/";
        public static readonly string[] CoreOverrides = {
            Brawler + "Fight_Idle.fbx", Brawler + "Hit_F.fbx", Brawler + "Hit_B.fbx", Brawler + "Guard_Hit.fbx",
            Fighting + "knockdown_A.fbx", Fighting + "getup_A.fbx"
        };

        public override uint GetVersion() => 1;

        void OnPreprocessAnimation()
        {
            int index = Array.FindIndex(CoreOverrides, path => string.Equals(path, assetPath, StringComparison.OrdinalIgnoreCase));
            if (index < 0) return;
            var importer = (ModelImporter)assetImporter;
            var clips = importer.clipAnimations;
            if (clips.Length == 0) clips = importer.defaultClipAnimations;
            foreach (var clip in clips)
            {
                // Extract XZ for the sole motor to discard. Preserve the authored fall/turn inside the pose.
                // Unity 6 renamed the three "Bake Into Pose" toggles. loopBlendPositionXZ/loopBlendPositionY/
                // loopBlendOrientation are the legacy serialized spellings; the public API is lockRootPositionXZ/
                // lockRootHeightY/lockRootRotation. Same three fields, same sense, and the values below are
                // Codex's own - only the identifiers changed, so "lock the root" == "bake into pose" as before.
                clip.lockRootPositionXZ = false;
                if (index >= 4) { clip.lockRootHeightY = true; clip.lockRootRotation = true; }
            }
            importer.clipAnimations = clips;
        }

        public static void ReimportCoreClips()
        {
            AssetDatabase.StartAssetEditing();
            try
            {
                foreach (string path in CoreOverrides) AssetDatabase.ImportAsset(path, ImportAssetOptions.ForceUpdate);
            }
            finally { AssetDatabase.StopAssetEditing(); }
        }
    }
}
