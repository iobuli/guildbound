"""Synthesizes the game's combat, magic, monster and ambience sounds into assets/audio/synth/*.wav.

Pure numpy (no samples, no licences): filtered noise, tones and envelopes. Each cue gets a few variants
(different random seeds) so repeats never sound identical; game/sfx.gd picks one at random and spreads
the pitch a little. Run from the guildbound folder:  python tools/make_sfx.py
"""
import math
import os
import wave
import zlib

import numpy as np

SR = 44100
OUT = os.path.join(os.path.dirname(__file__), "..", "assets", "audio", "synth")


# ---- building blocks -------------------------------------------------------------------------------

def t_axis(dur):
    return np.arange(int(SR * dur)) / SR


def env(dur, attack, decay_power=1.0, hold=0.0):
    """Attack ramp, optional hold, then a decay to silence shaped by decay_power (>1 = faster fall)."""
    t = t_axis(dur)
    a = np.clip(t / max(attack, 1e-4), 0, 1)
    d = np.clip(1 - (t - attack - hold) / max(dur - attack - hold, 1e-4), 0, 1) ** decay_power
    return a * np.where(t < attack + hold, 1.0, d)


def svf(x, cutoff, q=0.7, mode="band"):
    """State-variable filter with a per-sample cutoff (Hz): 'low', 'band' or 'high'."""
    cutoff = np.broadcast_to(np.asarray(cutoff, dtype=float), x.shape)
    low = band = 0.0
    out = np.empty_like(x)
    damp = 1.0 / q
    for i in range(len(x)):
        f = 2.0 * math.sin(math.pi * min(cutoff[i], SR * 0.45) / SR)
        high = x[i] - low - damp * band
        band += f * high
        low += f * band
        out[i] = band if mode == "band" else (low if mode == "low" else high)
    return out


def noise(dur, rng):
    return rng.uniform(-1, 1, int(SR * dur))


def sweep(dur, f0, f1, curve=1.0):
    t = t_axis(dur) / dur
    return f0 + (f1 - f0) * t ** curve


def tone(dur, freqs, decays, rng=None, detune=0.0):
    """Sum of decaying sines (metal, bells, chimes)."""
    t = t_axis(dur)
    out = np.zeros_like(t)
    for f, d in zip(freqs, decays):
        ff = f * (1 + (rng.uniform(-detune, detune) if rng is not None else 0))
        out += np.sin(2 * math.pi * ff * t + (rng.uniform(0, 6.28) if rng is not None else 0)) * np.exp(-t / d)
    return out


def chirp(dur, f0, f1):
    t = t_axis(dur)
    f = sweep(dur, f0, f1)
    return np.sin(2 * math.pi * np.cumsum(f) / SR)


def crackle(dur, rng, density=60.0):
    out = np.zeros(int(SR * dur))
    n = int(density * dur)
    for p in rng.integers(0, len(out) - 200, n):
        ln = rng.integers(20, 160)
        out[p:p + ln] += rng.uniform(-1, 1, ln) * np.exp(-np.arange(ln) / (ln / 4)) * rng.uniform(0.3, 1.0)
    return out


def fit(*parts):
    n = max(len(p) for p in parts)
    return [np.pad(p, (0, n - len(p))) for p in parts]


def mix(*parts):
    return sum(fit(*parts))


def finish(x, peak=0.89):
    x = x - np.mean(x)
    m = np.max(np.abs(x))
    if m > 0:
        x = x / m * peak
    fade = min(len(x), int(0.004 * SR))
    x[-fade:] *= np.linspace(1, 0, fade)
    return x


def save(name, x):
    os.makedirs(OUT, exist_ok=True)
    data = (finish(x) * 32767).astype("<i2")
    with wave.open(os.path.join(OUT, name + ".wav"), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(data.tobytes())


# ---- the sounds ------------------------------------------------------------------------------------

def swish(rng, dur, f0, f1, q, ring=0.0):
    """A swung weapon or limb cutting the air: a band of noise sweeping past."""
    e = np.sin(np.pi * np.clip(t_axis(dur) / dur, 0, 1)) ** 2
    x = svf(noise(dur, rng), sweep(dur, f0, f1, 0.7), q, "band") * e
    if ring:
        x += tone(dur, [rng.uniform(3200, 4200)], [0.08], rng) * e * ring
    return x


def hit_blade(rng):
    d = 0.35
    shink = svf(noise(0.12, rng), 5200, 1.5, "band") * env(0.12, 0.002, 2)
    metal = tone(d, [1850, 2930, 4410], [0.06, 0.04, 0.03], rng, 0.03) * 0.5
    thump = np.sin(2 * math.pi * sweep(0.18, 140, 60) * t_axis(0.18)) * env(0.18, 0.003, 2) * 0.8
    flesh = svf(noise(0.16, rng), 900, 0.8, "low") * env(0.16, 0.002, 2) * 0.7
    return mix(shink, metal, thump, flesh)


def clang(rng):
    base = rng.uniform(420, 520)
    ratios = [1.0, 2.76, 5.40, 8.93, 13.3]
    return mix(tone(1.0, [base * r for r in ratios], [0.45, 0.3, 0.18, 0.1, 0.06], rng, 0.01),
               svf(noise(0.05, rng), 6000, 1.0, "high") * env(0.05, 0.001, 2))


def fire_cast(rng):
    d = 0.6
    roar = svf(noise(d, rng), sweep(d, 300, 1400, 0.5), 0.6, "low") * env(d, 0.08, 1.5)
    return mix(roar, crackle(d, rng, 80) * env(d, 0.05, 1.2) * 0.5)


def fire_hit(rng):
    d = 1.0
    boom = np.sin(2 * math.pi * np.cumsum(sweep(d, 90, 35)) / SR) * env(d, 0.004, 3)
    blast = svf(noise(d, rng), sweep(d, 2500, 200, 0.4), 0.5, "low") * env(d, 0.002, 2.5)
    return mix(boom, blast * 0.9, crackle(d, rng, 50) * env(d, 0.05, 1.5) * 0.4)


def ice_cast(rng):
    d = 0.55
    notes = [1760, 2217, 2637, 3520]
    out = np.zeros(int(SR * d))
    for k, f in enumerate(notes):
        start = int(k * 0.06 * SR)
        s = tone(d - k * 0.06, [f, f * 2.01], [0.18, 0.08], rng, 0.004)
        out[start:start + len(s)] += s * 0.6
    return mix(out, svf(noise(d, rng), 7000, 1.2, "band") * env(d, 0.02, 2) * 0.3)


def ice_hit(rng):
    d = 0.7
    crack = svf(noise(d, rng), 4000, 0.7, "high") * env(d, 0.001, 4)
    pings = np.zeros(int(SR * d))
    for _ in range(14):
        p = int(rng.uniform(0, 0.25) * SR)
        s = tone(0.3, [rng.uniform(2500, 6500)], [0.05], rng) * rng.uniform(0.2, 0.6)
        pings[p:p + len(s)] += s[: len(pings) - p]
    return mix(crack, pings)


def lightning_cast(rng):
    d = 0.45
    t = t_axis(d)
    buzz = np.sign(np.sin(2 * math.pi * 110 * t)) * 0.3 + np.sign(np.sin(2 * math.pi * 167 * t)) * 0.2
    buzz = svf(buzz, 2000, 0.7, "low") * (0.6 + 0.4 * rng.uniform(0, 1, len(t))) * env(d, 0.01, 1.2)
    return mix(buzz, crackle(d, rng, 140) * 0.8)


def lightning_hit(rng):
    d = 1.4
    snap = svf(noise(0.08, rng), 5000, 0.6, "high") * env(0.08, 0.0005, 3) * 1.4
    rumble = svf(noise(d, rng), sweep(d, 600, 80, 0.5), 0.5, "low") * env(d, 0.03, 1.6)
    return mix(snap, rumble * 1.2, crackle(0.4, rng, 120) * 0.6)


def wind_cast(rng):
    d = 0.5
    return svf(noise(d, rng), sweep(d, 400, 3000, 0.6), 2.5, "band") * env(d, 0.12, 1.5)


def wind_hit(rng):
    d = 0.6
    return mix(svf(noise(d, rng), sweep(d, 1800, 250, 0.5), 1.5, "band") * env(d, 0.005, 2),
               np.sin(2 * math.pi * np.cumsum(sweep(0.25, 120, 50)) / SR) * env(0.25, 0.004, 2) * 0.7)


def arcane_cast(rng):
    d = 0.65
    t = t_axis(d)
    chord = sum(np.sin(2 * math.pi * f * (1 + 0.15 * t) * t) for f in (523, 659, 784, 988))
    trem = 0.6 + 0.4 * np.sin(2 * math.pi * 11 * t)
    return mix(chord * trem * env(d, 0.08, 1.5) * 0.4, svf(noise(d, rng), 6500, 2, "band") * env(d, 0.1, 1.5) * 0.25)


def arcane_hit(rng):
    d = 0.7
    bells = tone(d, [880, 1320, 1975, 2640], [0.25, 0.18, 0.12, 0.08], rng, 0.005)
    swell = svf(noise(0.2, rng), sweep(0.2, 500, 4000), 1.5, "band") * env(0.2, 0.15, 1)
    return mix(swell * 0.6, bells)


def shadow_cast(rng):
    d = 0.65
    t = t_axis(d)
    tone_ = np.sin(2 * math.pi * np.cumsum(sweep(d, 220, 90)) / SR) * env(d, 0.05, 1.3)
    hiss = svf(noise(d, rng), 3000, 0.8, "high") * env(d, 0.2, 1.2) * 0.4
    return mix(tone_ * 0.7, hiss, np.sin(2 * math.pi * 55 * t) * env(d, 0.1, 1.5) * 0.4)


def shadow_hit(rng):
    d = 0.6
    return mix(svf(noise(d, rng), sweep(d, 900, 150), 1.0, "low") * env(d, 0.01, 2),
               np.sin(2 * math.pi * np.cumsum(sweep(d, 160, 45)) / SR) * env(d, 0.005, 2.5) * 0.8)


def awaken(rng):
    d = 1.8
    t = t_axis(d)
    rise = sum(np.sin(2 * math.pi * f * (1 + 0.5 * (t / d) ** 2) * t) for f in (196, 247, 294, 392)) * 0.3
    rise *= np.clip(t / 1.2, 0, 1) ** 2 * np.clip((d - t) / 0.5, 0, 1)
    boom_at = int(1.15 * SR)
    boom = np.zeros_like(t)
    b = np.sin(2 * math.pi * np.cumsum(sweep(0.65, 70, 30)) / SR) * env(0.65, 0.004, 2.5)
    boom[boom_at:boom_at + len(b)] = b[: len(t) - boom_at]
    shimmer = svf(noise(d, rng), sweep(d, 2000, 9000), 3, "band") * np.clip(t / 1.2, 0, 1) * np.clip((d - t) / 0.6, 0, 1) * 0.3
    return mix(rise, boom * 1.2, shimmer)


def rank_up(rng):
    out = np.zeros(int(SR * 1.3))
    for k, f in enumerate([523.25, 659.25, 783.99, 1046.5]):
        start = int(k * 0.11 * SR)
        s = tone(1.3 - k * 0.11, [f, f * 2, f * 3.01], [0.5, 0.25, 0.12], rng, 0.002)
        out[start:start + len(s)] += s * 0.5
    return out


def slime(rng, kind):
    if kind == "hop":
        d = 0.18
        return np.sin(2 * math.pi * np.cumsum(sweep(d, 180, 420)) / SR) * env(d, 0.01, 1.5) * 0.6 + svf(noise(d, rng), 600, 3, "band") * env(d, 0.005, 2) * 0.3
    if kind == "hit":
        d = 0.25
        wob = np.sin(2 * math.pi * np.cumsum(220 + 120 * np.sin(2 * math.pi * 30 * t_axis(d))) / SR)
        return wob * env(d, 0.003, 2) + svf(noise(d, rng), 900, 2, "band") * env(d, 0.002, 2) * 0.6
    d = 0.5  # death: a wet pop
    popp = np.sin(2 * math.pi * np.cumsum(sweep(0.08, 900, 200)) / SR) * env(0.08, 0.001, 2)
    splat = svf(noise(d, rng), sweep(d, 1500, 300), 1.2, "band") * env(d, 0.01, 2)
    return mix(popp, splat)


def step_stone(rng):
    d = 0.09
    return mix(svf(noise(d, rng), 2600, 1.2, "band") * env(d, 0.001, 3), tone(d, [rng.uniform(180, 260)], [0.02], rng) * 0.5)


def ambience_wind(rng):
    d = 10.0
    t = t_axis(d)
    lfo = 0.55 + 0.45 * np.sin(2 * math.pi * t / d * 3 + 1.0) * np.sin(2 * math.pi * t / d * 2)
    x = svf(noise(d, rng), 380 + 260 * lfo, 0.9, "band") * lfo
    n = int(0.5 * SR)  # seamless loop: cross-fade the end into the start
    x[:n] = x[:n] * np.linspace(0, 1, n) + x[-n:] * np.linspace(1, 0, n)
    return x[:-n]


def bird(rng):
    out = np.zeros(int(SR * 0.6))
    p = 0
    for _ in range(rng.integers(2, 5)):
        d = rng.uniform(0.05, 0.11)
        f0 = rng.uniform(2500, 4200)
        s = chirp(d, f0, f0 * rng.uniform(0.6, 1.4)) * np.sin(np.pi * t_axis(d) / d)
        out[p:p + len(s)] += s[: len(out) - p]
        p += len(s) + int(rng.uniform(0.02, 0.06) * SR)
        if p >= len(out) - 100:
            break
    return out


CUES = {
    "swish_light": lambda r: swish(r, 0.16, 900, 3200, 1.6),
    "swish_blade": lambda r: swish(r, 0.26, 1400, 5200, 2.2, ring=0.25),
    "swish_heavy": lambda r: swish(r, 0.42, 300, 1600, 1.2),
    "swish_kick": lambda r: swish(r, 0.22, 500, 2000, 1.3),
    "hit_blade": hit_blade,
    "clang": clang,
    "fire_cast": fire_cast, "fire_hit": fire_hit,
    "ice_cast": ice_cast, "ice_hit": ice_hit,
    "lightning_cast": lightning_cast, "lightning_hit": lightning_hit,
    "wind_cast": wind_cast, "wind_hit": wind_hit,
    "arcane_cast": arcane_cast, "arcane_hit": arcane_hit,
    "shadow_cast": shadow_cast, "shadow_hit": shadow_hit,
    "awaken": awaken, "rank_up": rank_up,
    "slime_hop": lambda r: slime(r, "hop"), "slime_hit": lambda r: slime(r, "hit"), "slime_die": lambda r: slime(r, "die"),
    "step_stone": step_stone, "bird": bird,
}
VARIANTS = {"awaken": 1, "rank_up": 1, "bird": 4, "step_stone": 4}

if __name__ == "__main__":
    for name, fn in CUES.items():
        for v in range(VARIANTS.get(name, 3)):
            save("%s_%d" % (name, v), fn(np.random.default_rng(zlib.crc32(("%s/%d" % (name, v)).encode()))))
    save("ambience_wind", ambience_wind(np.random.default_rng(7)))
    print("wrote", len(os.listdir(OUT)), "files to", os.path.normpath(OUT))
