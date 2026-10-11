using UnityEngine;
using UnityEngine.Rendering;

namespace Guildbound.World
{
    // One streamed chunk: its GameObject, mesh, and the collider it gains when the viewer is close.
    //
    // Vertices come from sampling HeightField once per unique world position on a padded grid, and
    // normals are derived from that same padded grid by central difference. Two consequences worth
    // knowing: the field is sampled (res+2)^2 times instead of 5 x res^2, which is what keeps a chunk
    // inside the streaming budget; and the shading matches the mesh that was built, so a chunk does not
    // change appearance when its neighbours load. Normals do differ across an LOD boundary, because the
    // sample spacing differs there. That is inherent to LOD and is what the skirts hide.
    public sealed class TerrainChunk
    {
        public readonly Vector2Int Key;
        public int Lod { get; private set; } = -1;

        readonly GameObject root;
        readonly MeshFilter filter;
        readonly MeshRenderer meshRenderer;
        readonly MeshCollider meshCollider;

        public GameObject Root => root;
        public bool HasCollider { get; private set; }

        public TerrainChunk(Vector2Int key, Transform parent, Material material)
        {
            Key = key;
            root = new GameObject("Chunk " + key.x + "," + key.y);
            root.transform.SetParent(parent, false);
            root.transform.position = ChunkGrid.Origin(key);
            filter = root.AddComponent<MeshFilter>();
            meshRenderer = root.AddComponent<MeshRenderer>();
            if (material) meshRenderer.sharedMaterial = material;
            meshRenderer.shadowCastingMode = ShadowCastingMode.On;
            meshCollider = root.AddComponent<MeshCollider>();
            meshCollider.enabled = false;
        }

        // Rebuilds the mesh only when the LOD actually changes. Returns true when it did work.
        public bool Build(HeightField field, int lod)
        {
            if (lod == Lod) return false;
            Lod = lod;

            var old = filter.sharedMesh;
            var mesh = Build(field, Key, lod);
            filter.sharedMesh = mesh;
            if (old) Object.Destroy(old);
            // The collider still holds the previous mesh; drop it so it cannot reference a destroyed asset.
            meshCollider.sharedMesh = null;
            HasCollider = false;
            meshCollider.enabled = false;
            return true;
        }

        // MASTER 4.12: collision is only needed for the 3 x 3 chunks around each actor. Cooking a
        // MeshCollider costs milliseconds, so it is done for a handful of chunks, never for all of them.
        public void SetCollider(bool on)
        {
            if (on == HasCollider) return;
            HasCollider = on;
            if (on)
            {
                meshCollider.sharedMesh = filter.sharedMesh;
                meshCollider.enabled = true;
            }
            else
            {
                meshCollider.enabled = false;
                meshCollider.sharedMesh = null;
            }
        }

        public void Dispose()
        {
            if (filter && filter.sharedMesh) Object.Destroy(filter.sharedMesh);
            if (root) Object.Destroy(root);
        }

        public static Mesh Build(HeightField field, Vector2Int chunk, int lod)
        {
            int res = ChunkGrid.Resolution(lod);
            float spacing = ChunkGrid.Spacing(lod);
            Vector3 origin = ChunkGrid.Origin(chunk);

            // Padded sample grid: one ring of extra samples past every edge, so a central difference at
            // a border vertex uses real field values instead of clamping or reaching into the neighbour.
            int pad = res + 2;
            var heights = new float[pad * pad];
            for (int z = -1; z <= res; z++)
                for (int x = -1; x <= res; x++)
                    heights[(z + 1) * pad + (x + 1)] =
                        field.Height(origin.x + x * spacing, origin.z + z * spacing);

            int ring = 4 * (res - 1);                 // perimeter vertices, walked as a closed loop
            var vertices = new Vector3[res * res + ring];
            var normals = new Vector3[vertices.Length];
            var uvs = new Vector2[vertices.Length];

            for (int z = 0; z < res; z++)
            {
                for (int x = 0; x < res; x++)
                {
                    int i = z * res + x;
                    float h = heights[(z + 1) * pad + (x + 1)];
                    vertices[i] = new Vector3(x * spacing, h, z * spacing);

                    float hl = heights[(z + 1) * pad + x];
                    float hr = heights[(z + 1) * pad + (x + 2)];
                    float hd = heights[z * pad + (x + 1)];
                    float hu = heights[(z + 2) * pad + (x + 1)];
                    normals[i] = new Vector3(hl - hr, 2f * spacing, hd - hu).normalized;
                    uvs[i] = new Vector2((float)x / (res - 1), (float)z / (res - 1));
                }
            }

            int[] perimeter = Perimeter(res);
            for (int p = 0; p < ring; p++)
            {
                int source = perimeter[p];
                int i = res * res + p;
                vertices[i] = vertices[source] - Vector3.up * ChunkGrid.SkirtDepth;
                normals[i] = normals[source];
                uvs[i] = uvs[source];
            }

            int gridQuads = (res - 1) * (res - 1);
            var triangles = new int[(gridQuads + ring) * 6];

            int t = 0;
            for (int z = 0; z < res - 1; z++)
            {
                for (int x = 0; x < res - 1; x++)
                {
                    int i = z * res + x;
                    triangles[t++] = i;
                    triangles[t++] = i + res;
                    triangles[t++] = i + res + 1;
                    triangles[t++] = i;
                    triangles[t++] = i + res + 1;
                    triangles[t++] = i + 1;
                }
            }

            for (int p = 0; p < ring; p++)
            {
                int next = (p + 1) % ring;
                int a = perimeter[p], b = perimeter[next];
                int sa = res * res + p, sb = res * res + next;
                triangles[t++] = a;
                triangles[t++] = sa;
                triangles[t++] = sb;
                triangles[t++] = a;
                triangles[t++] = sb;
                triangles[t++] = b;
            }

            var mesh = new Mesh { name = "Chunk " + chunk.x + "," + chunk.y + " L" + lod };
            mesh.indexFormat = vertices.Length > 65000 ? IndexFormat.UInt32 : IndexFormat.UInt16;
            mesh.vertices = vertices;
            mesh.normals = normals;
            mesh.uv = uvs;
            mesh.triangles = triangles;
            mesh.RecalculateBounds();
            return mesh;
        }

        // The perimeter of a res x res grid as a closed loop, counter-clockwise seen from above:
        // bottom row left-to-right, right column bottom-to-top, top row right-to-left, left column down.
        static int[] Perimeter(int res)
        {
            int ring = 4 * (res - 1);
            var loop = new int[ring];
            int n = 0;
            for (int x = 0; x < res - 1; x++) loop[n++] = x;
            for (int z = 0; z < res - 1; z++) loop[n++] = z * res + (res - 1);
            for (int x = res - 1; x > 0; x--) loop[n++] = (res - 1) * res + x;
            for (int z = res - 1; z > 0; z--) loop[n++] = z * res;
            return loop;
        }
    }
}
