using System.Text.Json;
using CUE4Parse.FileProvider;
using CUE4Parse.UE4.Versions;
using CUE4Parse.Encryption.Aes;
using CUE4Parse.UE4.Objects.Core.Misc;
using CUE4Parse.Compression;
using CUE4Parse.MappingsProvider.Usmap;
using CUE4Parse.UE4.Assets.Exports.Animation;
using CUE4Parse.UE4.Assets.Exports.StaticMesh;
using CUE4Parse_Conversion.Animations;
using Serilog;

Log.Logger = new LoggerConfiguration().MinimumLevel.Warning().WriteTo.Console().CreateLogger();
CUE4Parse.CUE4ParseLog.UseLogger(Log.Logger);

if (args.Length < 2)
{
    Console.Error.WriteLine("Usage: ArchiveProbe PAK_DIRECTORY REPORT.json [path-filter]");
    return 2;
}
using var provider = new DefaultFileProvider(args[0], SearchOption.TopDirectoryOnly,
    new VersionContainer(EGame.GAME_UE5_1));
provider.Initialize();
provider.Mount();
var suppliedKey = Environment.GetEnvironmentVariable("KEV_UNREAL_AES_KEY");
if (!string.IsNullOrWhiteSpace(suppliedKey))
    provider.SubmitKey(new FGuid(), new FAesKey(suppliedKey));
if (args.Length > 2 && args[2] == "--mesh")
{
    OodleHelper.Initialize();
    provider.MappingsContainer = new FileUsmapTypeMappingsProvider(Environment.GetEnvironmentVariable("KEV_UNREAL_MAPPINGS")!);
    provider.PostMount();
    var mesh = provider.LoadPackage(args[3]).GetExports().OfType<UStaticMesh>().First();
    var lod = mesh.RenderData!.LODs!.First(l => l.PositionVertexBuffer?.Verts.Length > 0 && l.IndexBuffer != null);
    var vertices = lod.PositionVertexBuffer!.Verts.Select(p => new[] { p.X * 0.01f, p.Z * 0.01f, p.Y * 0.01f }).ToArray();
    var indices = Enumerable.Range(0, lod.IndexBuffer!.Length).Select(i => lod.IndexBuffer[i]).ToArray();
    File.WriteAllText(args[1], JsonSerializer.Serialize(new { source = args[3], vertices, indices }));
    Console.WriteLine($"Mesh: {vertices.Length} vertices, {indices.Length / 3} triangles");
    return 0;
}
if (args.Length > 2 && (args[2] == "--clip" || args[2] == "--worker"))
{
    OodleHelper.Initialize();
    var mappings = Environment.GetEnvironmentVariable("KEV_UNREAL_MAPPINGS");
    if (!string.IsNullOrWhiteSpace(mappings))
        provider.MappingsContainer = new FileUsmapTypeMappingsProvider(mappings);
    provider.PostMount();
    var skeletonPath = "SparkingZERO/Content/SS/Characters/9999_base/SKEL_9999_base.uasset";
    var masterSkeleton = provider.LoadPackage(skeletonPath).GetExports().OfType<USkeleton>().First();
    if (args[2] == "--worker")
    {
        Console.WriteLine("KEV_READY");
        string? line;
        while ((line = Console.ReadLine()) != null)
        {
            try
            {
                using var request = JsonDocument.Parse(line);
                var path = request.RootElement.GetProperty("source").GetString()!;
                var output = request.RootElement.GetProperty("output").GetString()!;
                Decode(provider, masterSkeleton, path, output);
                Console.WriteLine("KEV_RESULT " + JsonSerializer.Serialize(new { ok = true, source = path, output }));
            }
            catch (Exception error)
            {
                Console.WriteLine("KEV_RESULT " + JsonSerializer.Serialize(new { ok = false, error = error.Message }));
            }
        }
        return 0;
    }
    Decode(provider, masterSkeleton, args[3], args[1]);
    return 0;
}
var filter = args.Length > 2 ? args[2] : "";
var files = provider.Files.Keys.Where(p => p.Contains(filter, StringComparison.OrdinalIgnoreCase))
    .OrderBy(p => p).ToArray();
var report = new
{
    archiveDirectory = Path.GetFullPath(args[0]),
    mountedFileCount = provider.Files.Count,
    requiredKeyCount = provider.RequiredKeys.Count,
    mountedArchives = provider.MountedVfs.Select(v => new { v.Name, v.IsEncrypted }).ToArray(),
    unavailableArchives = provider.UnloadedVfs.Select(v => new { v.Name, v.IsEncrypted }).ToArray(),
    filter,
    files
};
File.WriteAllText(args[1], JsonSerializer.Serialize(report, new JsonSerializerOptions { WriteIndented = true }));
Console.WriteLine($"Mounted files: {provider.Files.Count}; required keys: {provider.RequiredKeys.Count}; matches: {files.Length}");
return provider.Files.Count == 0 ? 1 : 0;

static void Decode(DefaultFileProvider provider, USkeleton masterSkeleton, string source, string output)
{
    var asset = provider.LoadPackage(source).GetExports().OfType<UAnimSequence>().FirstOrDefault()
        ?? throw new InvalidOperationException("No UAnimSequence in " + source);
    Console.WriteLine($"Master skeleton: {masterSkeleton.BoneCount} bones; clip: {asset.NumFrames} frames; {asset.SequenceLength}s");
    var converted = masterSkeleton.ConvertAnims(asset);
    var sequence = converted.Sequences[0];
    var skeleton = converted.Skeleton.ReferenceSkeleton;
    var wanted = new HashSet<string> { "NULL", "WAIST", "SPINE1", "SPINE2", "SPINE3", "NECK", "HEAD" };
    foreach (var side in new[] { "L", "R" })
    {
        foreach (var name in new[] { "THIGH", "CLANK", "TOE1", "TOE2", "CLAVICLE", "SHOULDER", "ELBOW", "WRIST" })
            wanted.Add(name + "_" + side);
        foreach (var finger in new[] { "THUMB", "FORE", "MIDDLE", "MEDICINAL", "LITTLE" })
            for (var i = 1; i <= 3; i++) wanted.Add($"F_{finger}{i}_{side}");
    }
    var keep = new HashSet<int>();
    for (var i = 0; i < skeleton.FinalRefBoneInfo.Length; i++)
        if (wanted.Contains(skeleton.FinalRefBoneInfo[i].Name.Text))
            for (var j = i; j >= 0 && keep.Add(j); j = skeleton.FinalRefBoneInfo[j].ParentIndex) { }
    var indices = keep.OrderBy(i => i).ToArray();
    var compact = indices.Select((original, i) => (original, i)).ToDictionary(x => x.original, x => x.i);
    var bones = indices.Select(i => new {
        name = skeleton.FinalRefBoneInfo[i].Name.Text,
        parent = compact.GetValueOrDefault(skeleton.FinalRefBoneInfo[i].ParentIndex, -1),
        rest = Transform(skeleton.FinalRefBonePose[i])
    }).ToArray();
    var frames = new List<object>();
    for (var frame = 0; frame < sequence.NumFrames; frame++)
    {
        var pose = indices.Select(i => skeleton.FinalRefBonePose[i]).ToArray();
        for (var i = 0; i < indices.Length; i++)
            if (indices[i] < sequence.Tracks.Count)
                sequence.Tracks[indices[i]].GetBoneTransform(frame, sequence.NumFrames,
                    ref pose[i].Rotation, ref pose[i].Translation, ref pose[i].Scale3D);
        frames.Add(new { t = frame * asset.SequenceLength / Math.Max(sequence.NumFrames - 1, 1),
            pose = pose.Select(Transform).ToArray() });
    }
    File.WriteAllText(output, JsonSerializer.Serialize(new {
        source, name = asset.Name, duration = asset.SequenceLength,
        framesPerSecond = sequence.FramesPerSecond, bones, frames
    }));
    Console.WriteLine($"Decoded {asset.Name}: {bones.Length} bones, {frames.Count} frames, {asset.SequenceLength}s");
}

static object Transform(CUE4Parse.UE4.Objects.Core.Math.FTransform t) => new {
    q = new[] { t.Rotation.X, t.Rotation.Y, t.Rotation.Z, t.Rotation.W },
    p = new[] { t.Translation.X, t.Translation.Y, t.Translation.Z },
    s = new[] { t.Scale3D.X, t.Scale3D.Y, t.Scale3D.Z }
};
