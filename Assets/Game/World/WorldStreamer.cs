using System.Collections.Generic;
using System.Diagnostics;
using UnityEngine;

namespace Guildbound.World
{
    // Keeps a square of terrain chunks loaded around the viewer and unloads the rest (MASTER 4.12).
    //
    // Two budgets are enforced, both from MASTER 4.12: mesh building stays inside frameBudgetMs per
    // frame, and collision cooking is limited to one chunk per frame and only inside colliderRadius.
    // Generation is synchronous and budgeted rather than threaded. That is deliberate for the first
    // pass - it is deterministic, it is measurable, and it keeps the frame honest. Moving the build to
    // a worker needs Mesh.AllocateWritableMeshData, which is a separate change with its own test.
    [DefaultExecutionOrder(-100)]
    public sealed class WorldStreamer : MonoBehaviour
    {
        [Tooltip("What the world is streamed around. Falls back to the main camera.")]
        public Transform viewer;
        public Material material;

        public int seed = 1337;

        [Tooltip("Chunks out from the viewer that stay loaded, as a Chebyshev radius.")]
        public int viewRadius = 4;

        [Tooltip("Mesh building allowed per frame, in milliseconds. MASTER 4.12 gives streaming 4 ms.")]
        public float frameBudgetMs = 4f;

        [Tooltip("Chunks out from the viewer that get a collider. 1 = the 3 x 3 around the viewer.")]
        public int colliderRadius = 1;

        readonly Dictionary<Vector2Int, TerrainChunk> live = new Dictionary<Vector2Int, TerrainChunk>();
        readonly Stopwatch budget = new Stopwatch();
        readonly List<Vector2Int> expired = new List<Vector2Int>();

        HeightField field;
        Transform chunks;

        public HeightField Field => field;
        public int LiveChunks => live.Count;
        public int ChunksWithColliders { get; private set; }

        void Awake()
        {
            field = new HeightField(seed);
            chunks = new GameObject("Terrain Chunks").transform;
            chunks.SetParent(transform, false);
            if (!viewer && Camera.main) viewer = Camera.main.transform;
        }

        void Update()
        {
            if (!viewer) return;
            var centre = ChunkGrid.ChunkOf(viewer.position);

            UnloadOutside(centre);
            BuildWithinBudget(centre);
            UpdateColliders(centre);
        }

        void UnloadOutside(Vector2Int centre)
        {
            int keep = viewRadius + 1;   // one ring of slack, so a viewer on a border does not thrash
            expired.Clear();
            foreach (var pair in live)
                if (ChunkGrid.Ring(pair.Key, centre) > keep) expired.Add(pair.Key);
            foreach (var key in expired)
            {
                live[key].Dispose();
                live.Remove(key);
            }
        }

        // Walks outward in rings so the nearest missing chunk is always built first, and stops as soon
        // as the frame budget is spent. Ring order means no distance sort and no per-frame allocation.
        void BuildWithinBudget(Vector2Int centre)
        {
            budget.Restart();
            for (int ring = 0; ring <= viewRadius; ring++)
            {
                foreach (var key in Ring(centre, ring))
                {
                    if (budget.Elapsed.TotalMilliseconds >= frameBudgetMs) return;
                    if (!live.TryGetValue(key, out var chunk))
                    {
                        chunk = new TerrainChunk(key, chunks, material);
                        live.Add(key, chunk);
                    }
                    chunk.Build(field, ChunkGrid.LodForRing(ring));
                }
            }
        }

        // MASTER 4.12: collision only for the 3 x 3 around each actor. One cook per frame, nearest
        // first, so a fast viewer never stalls the frame on collider cooking.
        void UpdateColliders(Vector2Int centre)
        {
            ChunksWithColliders = 0;
            for (int ring = 0; ring <= colliderRadius; ring++)
                foreach (var key in Ring(centre, ring))
                    if (live.TryGetValue(key, out var chunk) && chunk.HasCollider) ChunksWithColliders++;

            for (int ring = 0; ring <= colliderRadius; ring++)
            {
                foreach (var key in Ring(centre, ring))
                {
                    if (!live.TryGetValue(key, out var chunk) || chunk.HasCollider) continue;
                    if (chunk.Lod < 0) continue;
                    chunk.SetCollider(true);
                    ChunksWithColliders++;
                    return;   // one collider per frame
                }
            }
        }

        // The chunks at exactly this Chebyshev ring, in a fixed order so builds are reproducible.
        static IEnumerable<Vector2Int> Ring(Vector2Int centre, int ring)
        {
            if (ring == 0)
            {
                yield return centre;
                yield break;
            }
            for (int dx = -ring; dx <= ring; dx++)
            {
                yield return new Vector2Int(centre.x + dx, centre.y + ring);
                yield return new Vector2Int(centre.x + dx, centre.y - ring);
            }
            for (int dz = -ring + 1; dz <= ring - 1; dz++)
            {
                yield return new Vector2Int(centre.x + ring, centre.y + dz);
                yield return new Vector2Int(centre.x - ring, centre.y + dz);
            }
        }

        // Ground height under a world position, whether or not that chunk is currently loaded.
        public float HeightAt(Vector3 world) => field.Height(world.x, world.z);

        void OnDestroy()
        {
            foreach (var pair in live) pair.Value.Dispose();
            live.Clear();
        }
    }
}
