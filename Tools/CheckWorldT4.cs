using System;
using System.Diagnostics;
using System.IO;
using System.Text;
using Guildbound.World;
using UnityEngine;

// MASTER 7 / T4: skeleton + procedural world - deterministic seed test, chunk streaming under budget,
// walkable terrain. This is the deterministic-seed and walkability half of that gate. It is READ-ONLY:
// it creates no GameObjects, touches no scene, and writes only Documentation/checks/world/t4-world.txt.
// Every failure throws; nothing is reported as a pass it did not measure.
public static class CheckWorldT4
{
    public static string Execute()
    {
        var report = new StringBuilder("World T4 - " + Application.unityVersion + "\n");

        // --- 1. Determinism: the same seed must give the same world, twice over, bit for bit. -------
        var a = new HeightField(1337);
        var b = new HeightField(1337);
        int mismatchedHeights = 0, mismatchedNormals = 0;
        const int probes = 2000;
        var rng = new DeterministicProbe(1337);
        for (int i = 0; i < probes; i++)
        {
            float x = rng.NextFloat(-2000f, 2000f), z = rng.NextFloat(-2000f, 2000f);
            if (a.Height(x, z) != b.Height(x, z)) mismatchedHeights++;
            if (a.Normal(x, z) != b.Normal(x, z)) mismatchedNormals++;
        }
        report.AppendLine("same seed, 2 instances, " + probes + " probes: height mismatches " + mismatchedHeights
            + ", normal mismatches " + mismatchedNormals);

        // The seed must actually do something, or the test above is vacuous.
        var c = new HeightField(1338);
        int differing = 0;
        rng = new DeterministicProbe(1337);
        for (int i = 0; i < probes; i++)
        {
            float x = rng.NextFloat(-2000f, 2000f), z = rng.NextFloat(-2000f, 2000f);
            if (a.Height(x, z) != c.Height(x, z)) differing++;
        }
        report.AppendLine("seed 1337 vs 1338, " + probes + " probes: differing heights " + differing
            + " (" + (100f * differing / probes).ToString("F1") + "%)");

        // --- 2. Chunk streaming: same size, same LOD lattice, and a measured build cost. -----------
        report.AppendLine("chunk size " + ChunkGrid.ChunkSize + " m, LODs " + ChunkGrid.LodCount);
        for (int lod = 0; lod < ChunkGrid.LodCount; lod++)
            report.AppendLine("  LOD" + lod + " resolution " + ChunkGrid.Resolution(lod)
                + " spacing " + ChunkGrid.Spacing(lod).ToString("F2") + " m");

        var chunk = new Vector2Int(0, 0);

        // Warm up before timing. A single cold build measures JIT, not the sampler - the first sample of
        // an unwarmed chunk in this project reads about 7 ms against a 4.1 ms steady state. The cold
        // figure is reported separately rather than hidden, because it is a real startup cost.
        var cold = Stopwatch.StartNew();
        var warm = TerrainChunk.Build(a, new Vector2Int(99, 99), 0);
        cold.Stop();
        double coldMs = cold.Elapsed.TotalMilliseconds;
        UnityEngine.Object.DestroyImmediate(warm);

        const int timedRuns = 9;
        var samples = new double[timedRuns];
        for (int i = 0; i < timedRuns; i++)
        {
            var timer = Stopwatch.StartNew();
            var m = TerrainChunk.Build(a, chunk, 0);
            timer.Stop();
            samples[i] = timer.Elapsed.TotalMilliseconds;
            UnityEngine.Object.DestroyImmediate(m);
        }
        Array.Sort(samples);
        double medianMs = samples[timedRuns / 2];
        double totalMs = 0;
        foreach (var s in samples) totalMs += s;
        double meanMs = totalMs / timedRuns;

        var mesh0 = TerrainChunk.Build(a, chunk, 0);
        var mesh0b = TerrainChunk.Build(new HeightField(1337), chunk, 0);

        // Mesh-to-mesh determinism: two independent builds of the same chunk must agree exactly.
        int vertexMismatches = 0;
        if (mesh0.vertexCount != mesh0b.vertexCount) vertexMismatches = -1;
        else
        {
            var va = mesh0.vertices; var vb = mesh0b.vertices;
            for (int i = 0; i < va.Length; i++) if (va[i] != vb[i]) vertexMismatches++;
            var ta = mesh0.triangles; var tb = mesh0b.triangles;
            if (ta.Length != tb.Length) vertexMismatches = -1;
            else for (int i = 0; i < ta.Length; i++) if (ta[i] != tb[i]) vertexMismatches++;
        }
        report.AppendLine("LOD0 mesh: " + mesh0.vertexCount + " vertices, " + (mesh0.triangles.Length / 3)
            + " triangles");
        report.AppendLine("LOD0 build: cold " + coldMs.ToString("F2") + " ms (JIT), steady state median "
            + medianMs.ToString("F2") + " ms, mean " + meanMs.ToString("F2") + " ms over " + timedRuns + " runs");
        report.AppendLine("two independent builds of the same chunk: " + vertexMismatches + " mismatches");

        double ringTotal = 0;
        var ringTimer = Stopwatch.StartNew();
        for (int dz = -1; dz <= 1; dz++)
            for (int dx = -1; dx <= 1; dx++)
            {
                var m = TerrainChunk.Build(a, new Vector2Int(dx, dz), 0);
                UnityEngine.Object.DestroyImmediate(m);
            }
        ringTimer.Stop();
        ringTotal = ringTimer.Elapsed.TotalMilliseconds;
        report.AppendLine("3 x 3 ring of LOD0 chunks: " + ringTotal.ToString("F1") + " ms total, "
            + (ringTotal / 9.0).ToString("F1") + " ms per chunk");

        UnityEngine.Object.DestroyImmediate(mesh0);
        UnityEngine.Object.DestroyImmediate(mesh0b);

        // --- 3. Walkable terrain: slope is the number that decides this, so measure it. ------------
        var terrain = new HeightField(1337);
        const int sampleCount = 20000;
        var probe = new DeterministicProbe(99);
        int walkable = 0, steep = 0, cliff = 0;
        float maxSlope = 0f, minH = float.MaxValue, maxH = float.MinValue;
        for (int i = 0; i < sampleCount; i++)
        {
            float x = probe.NextFloat(-500f, 500f), z = probe.NextFloat(-500f, 500f);
            float h = terrain.Height(x, z);
            if (h < minH) minH = h;
            if (h > maxH) maxH = h;
            float slope = Mathf.Acos(Mathf.Clamp(terrain.Normal(x, z).y, -1f, 1f)) * Mathf.Rad2Deg;
            if (slope > maxSlope) maxSlope = slope;
            if (slope <= 40f) walkable++;
            else if (slope <= 60f) steep++;
            else cliff++;
        }
        report.AppendLine("height range " + minH.ToString("F1") + " .. " + maxH.ToString("F1") + " m over a 1 km square");
        report.AppendLine("slope: " + (100f * walkable / sampleCount).ToString("F1") + "% <= 40 deg (walkable), "
            + (100f * steep / sampleCount).ToString("F1") + "% 40-60 deg, "
            + (100f * cliff / sampleCount).ToString("F1") + "% > 60 deg; max " + maxSlope.ToString("F1") + " deg");

        // --- Verdict ---------------------------------------------------------------------------------
        bool determinism = mismatchedHeights == 0 && mismatchedNormals == 0 && vertexMismatches == 0;
        bool seedMatters = differing > probes / 2;
        bool budget = meanMs < 4.0 && (ringTotal / 9.0) < 4.5;
        // Walkable means two things at once, and the earlier version of this check only tested the
        // first: a majority of the surface can be walked, AND there is terrain worth walking - relief
        // and real slope somewhere. A flat plain scores 100% walkable and is not a world.
        bool walkability = walkable >= sampleCount * 0.6f && maxH - minH >= 40f && maxSlope >= 25f;

        report.AppendLine(determinism
            ? "PASS: deterministic - same seed, identical heights, normals and mesh vertices."
            : "FAIL: not deterministic across two instances of the same seed.");
        report.AppendLine(seedMatters
            ? "PASS: the seed changes the world (>50% of probes differ)."
            : "FAIL: the seed barely changes the world; the determinism test is vacuous.");
        report.AppendLine(budget
            ? "PASS: one LOD0 chunk inside the 4 ms streaming budget from MASTER 4.12."
            : "FAIL: chunk build exceeds the 4 ms streaming budget.");
        report.AppendLine(walkability
            ? "PASS: walkable world - >=60% at or under 40 deg, relief >= 40 m, steepest >= 25 deg."
            : "FAIL: not a walkable world as measured (needs >=60% under 40 deg, >=40 m relief, >=25 deg max).");

        Directory.CreateDirectory("Documentation/checks/world");
        File.WriteAllText("Documentation/checks/world/t4-world.txt", report.ToString());

        bool pass = determinism && seedMatters && budget && walkability;
        if (!pass) throw new InvalidOperationException(report.ToString());
        return report.ToString();
    }

    // Integer-hash random: the check must not use UnityEngine.Random, or the probe set itself would
    // differ between runs and the determinism result would be meaningless.
    struct DeterministicProbe
    {
        uint state;
        public DeterministicProbe(int seed) => state = seed == 0 ? 0x9E3779B9u : (uint)seed;
        public float NextFloat(float min, float max)
        {
            state ^= state << 13; state ^= state >> 17; state ^= state << 5;
            return min + (max - min) * ((state >> 8) / 16777216f);
        }
    }
}
