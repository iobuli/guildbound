using System.IO;
using System.Text.RegularExpressions;
using UnityEditor;
using UnityEngine;

namespace Guildbound.UI.Editor
{
    // Import settings for the HUD kit: frames get the exact 9-slice borders recorded by tools/build_hud_kit.py
    // (kit.json, texture pixels, L B R T) and pixelsPerUnit 200 (2x textures shown at reference size); icons 100.
    public class HudKitImporter : AssetPostprocessor
    {
        const string Root = "Assets/Game/UI/Kit/";
        const string Sprites = Root + "Sprites/";

        void OnPreprocessTexture()
        {
            if (!assetPath.StartsWith(Root)) return;
            var ti = (TextureImporter)assetImporter;
            ti.textureType = TextureImporterType.Sprite;
            ti.spriteImportMode = SpriteImportMode.Single;
            ti.alphaIsTransparency = true;
            ti.mipmapEnabled = false;
            ti.wrapMode = TextureWrapMode.Clamp;
            ti.filterMode = FilterMode.Bilinear;
            ti.textureCompression = TextureImporterCompression.Uncompressed;
            ti.spritePixelsPerUnit = 100;
            if (!assetPath.StartsWith(Sprites)) return;

            ti.spritePixelsPerUnit = 200;
            var kit = File.ReadAllText(Sprites + "kit.json");
            var name = Path.GetFileNameWithoutExtension(assetPath);
            var m = Regex.Match(kit, "\"" + Regex.Escape(name) + "\": \\{.*?\"border_tex_px_LBRT\": \\[\\s*(\\d+),\\s*(\\d+),\\s*(\\d+),\\s*(\\d+)\\s*\\]", RegexOptions.Singleline);
            if (m.Success)
                ti.spriteBorder = new Vector4(int.Parse(m.Groups[1].Value), int.Parse(m.Groups[2].Value), int.Parse(m.Groups[3].Value), int.Parse(m.Groups[4].Value));
            else
                Debug.LogWarning("HUD_KIT no kit.json entry for " + assetPath);
        }
    }
}
