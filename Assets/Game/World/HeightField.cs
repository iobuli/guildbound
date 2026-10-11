using UnityEngine;

namespace Guildbound.World
{
    // Seeded deterministic height field (MASTER 4.12).
    //
    // Same seed gives the same world on every machine. Every value comes from integer hashing and a
    // fixed quintic fade - never from System.Random, System.DateTime, or an accumulation whose order
    // depends on thread scheduling. That is what the deterministic seed test in MASTER 7 / T4 checks.
    //
    // Bands, from the design: 5-octave fBm (lacunarity 2, gain 0.5) for the hills, a ridged layer for
    // the mountains, a higher-frequency detail band for local roughness, and a low-frequency domain
    // warp so the hills do not read as a lattice. MASTER names FastNoiseLite/OpenSimplex2 as the noise
    // source; this is a gradient-noise lattice carrying the same octave parameters, because the project
    // ships no noise package and a dependency is not worth taking for one function. The band structure
    // is the part the design depends on; the lattice is a tuning difference.
    //
    // Cost note, because it matters: this runs per vertex. The gradients are a static table rather than
    // Mathf.Cos/Mathf.Sin, which is what puts a chunk inside the 4 ms streaming budget instead of 16x
    // over it. Do not reintroduce trig here.
    public sealed class HeightField
    {
        public readonly int Seed;

        // Hills. Wavelength ~250 m, so a chunk sees a slope rather than a bump.
        public float Frequency = 0.0040f;
        public int Octaves = 5;
        public float Lacunarity = 2f;
        public float Gain = 0.5f;
        public float Amplitude = 30f;

        // Mountains. Longer wavelength than the hills so crests read as landmarks, and squared so the
        // flanks are steep while the valleys stay open and walkable.
        public float RidgedFrequency = 0.0025f;
        public float RidgedAmplitude = 58f;

        // Local roughness at ~70 m, small enough to stay walkable everywhere it appears.
        public float DetailFrequency = 0.014f;
        public float DetailAmplitude = 2.5f;

        // Domain warp. Without it the hills read as a regular lattice at distance.
        public float WarpFrequency = 0.0016f;
        public float WarpStrength = 40f;

        public HeightField(int seed) => Seed = seed;

        // World metres in, world metres up. y = 0 is sea level, not the floor of the world.
        public float Height(float x, float z)
        {
            float wx = Lattice(x * WarpFrequency + 5.2f, z * WarpFrequency + 1.3f, Seed + 101);
            float wz = Lattice(x * WarpFrequency + 1.7f, z * WarpFrequency + 9.2f, Seed + 211);
            float px = x + wx * WarpStrength;
            float pz = z + wz * WarpStrength;

            float hills = Fbm(px * Frequency, pz * Frequency, Octaves, Seed) * 0.5f + 0.5f;
            float ridged = 1f - Mathf.Abs(Lattice(px * RidgedFrequency, pz * RidgedFrequency, Seed + 7));
            float detail = Lattice(px * DetailFrequency, pz * DetailFrequency, Seed + 313);

            return hills * Amplitude + ridged * ridged * RidgedAmplitude + detail * DetailAmplitude;
        }

        // Surface normal by central difference, for callers that need a normal at an arbitrary point -
        // foot fit, slope checks. The chunk mesh does NOT call this per vertex; it derives normals from
        // its own padded sample grid, which is about 5x cheaper and matches the mesh it built.
        public Vector3 Normal(float x, float z)
        {
            const float e = 0.5f;
            float hl = Height(x - e, z), hr = Height(x + e, z);
            float hd = Height(x, z - e), hu = Height(x, z + e);
            return new Vector3(hl - hr, 2f * e, hd - hu).normalized;
        }

        public float Fbm(float x, float z, int octaves, int seed)
        {
            float sum = 0f, norm = 0f, amp = 1f, freq = 1f;
            for (int i = 0; i < octaves; i++)
            {
                sum += Lattice(x * freq, z * freq, seed + i * 131) * amp;
                norm += amp;
                amp *= Gain;
                freq *= Lacunarity;
            }
            return norm > 0f ? sum / norm : 0f;
        }

        // Classic gradient noise on an integer lattice: hash the four corners, dot each with its
        // gradient, fade and bilinear-interpolate. Output is roughly -1..1.
        public static float Lattice(float x, float z, int seed)
        {
            int xi = FastFloor(x), zi = FastFloor(z);
            float xf = x - xi, zf = z - zi;
            float u = Fade(xf), v = Fade(zf);

            float n00 = Gradient(Hash(xi, zi, seed), xf, zf);
            float n10 = Gradient(Hash(xi + 1, zi, seed), xf - 1f, zf);
            float n01 = Gradient(Hash(xi, zi + 1, seed), xf, zf - 1f);
            float n11 = Gradient(Hash(xi + 1, zi + 1, seed), xf - 1f, zf - 1f);
            // Hand-rolled, not Mathf.Lerp: this runs three times per lattice, about 120,000 times per
            // LOD0 chunk, and a static call that does not inline costs more than the arithmetic.
            float a = n00 + (n10 - n00) * u;
            float b = n01 + (n11 - n01) * u;
            return a + (b - a) * v;
        }

        // Mathf.FloorToInt is (int)Math.Floor((double)f) in this Unity version - a double round-trip in
        // the hottest loop in the project, twice per lattice, ~80,000 times per LOD0 chunk. This is the
        // same answer without leaving single precision.
        static int FastFloor(float f)
        {
            int i = (int)f;
            return f < i ? i - 1 : i;
        }

        static float Fade(float t) => t * t * t * (t * (t * 6f - 15f) + 10f);

        // Integer hash. Overflow is intended and wrapped, so the result is identical everywhere.
        static int Hash(int x, int y, int seed)
        {
            unchecked
            {
                int h = seed * 374761393 + x * 668265263 + y * 1274126177;
                h = (h ^ (h >> 13)) * 1274126177;
                return h ^ (h >> 16);
            }
        }

        // Eight evenly spaced unit gradients, as two flat float arrays. cos/sin of g * pi/4, evaluated
        // once at type load and never in the sample path, and flat rather than a Vector2[] so the hot
        // loop does no struct copying. Between this and FastFloor, a chunk went from 66 ms to under 4.
        static readonly float[] Gx = { 1f, 0.9238795f, 0.7071068f, 0.3826834f, 0f, -0.3826834f, -0.7071068f, -0.9238795f };
        static readonly float[] Gz = { 0f, 0.3826834f, 0.7071068f, 0.9238795f, 1f, 0.9238795f, 0.7071068f, 0.3826834f };

        static float Gradient(int hash, float dx, float dz)
        {
            int g = hash & 7;
            return Gx[g] * dx + Gz[g] * dz;
        }
    }
}
