using UnityEngine;

namespace Guildbound.World
{
    // Chunk grid maths (MASTER 4.12): 64 m x 64 m chunks, 65 x 65 vertices at LOD0 - a 1 m spacing -
    // halved per LOD across 3 LODs. Pure functions, no state, so both the streamer and the tests can
    // use it without agreeing on anything but these numbers.
    public static class ChunkGrid
    {
        public const float ChunkSize = 64f;
        public const int LodCount = 3;
        public const int BaseResolution = 65;      // 65 x 65 vertices = 64 x 64 quads at 1 m
        public const float SkirtDepth = 4f;        // hides cracks between neighbouring LODs

        // LOD0 -> 65, LOD1 -> 33, LOD2 -> 17. Always an odd count, so the chunk edges stay shared.
        public static int Resolution(int lod) => ((BaseResolution - 1) >> lod) + 1;

        // Vertex spacing in metres at this LOD.
        public static float Spacing(int lod) => ChunkSize / (Resolution(lod) - 1);

        public static Vector2Int ChunkOf(float x, float z)
            => new Vector2Int(Mathf.FloorToInt(x / ChunkSize), Mathf.FloorToInt(z / ChunkSize));

        public static Vector2Int ChunkOf(Vector3 world) => ChunkOf(world.x, world.z);

        // South-west corner of the chunk, on the y = 0 plane.
        public static Vector3 Origin(Vector2Int chunk)
            => new Vector3(chunk.x * ChunkSize, 0f, chunk.y * ChunkSize);

        public static Vector3 Centre(Vector2Int chunk)
            => new Vector3((chunk.x + 0.5f) * ChunkSize, 0f, (chunk.y + 0.5f) * ChunkSize);

        // Chebyshev ring distance from a to b: the number of chunks between them, 0 when equal.
        public static int Ring(Vector2Int a, Vector2Int b)
            => Mathf.Max(Mathf.Abs(a.x - b.x), Mathf.Abs(a.y - b.y));

        // LOD by ring distance: the chunk under the viewer at full detail, then one step out, then
        // the rest coarse. Clamped so a large view radius cannot index past LodCount.
        public static int LodForRing(int ring)
        {
            if (ring <= 1) return 0;
            if (ring <= 2) return 1;
            return 2;
        }
    }
}
