#!/usr/bin/env python3
"""Synthesize Thoral's boss theme: assets/audio/synth/boss_thoral.wav (mono, 22.05 kHz, 16-bit).

A 32 s seamless loop at 96 bpm (8 bars of 4/4 ... 12.8 bars; the loop is cut on a bar line):
- taiko-like drums: a deep, pitch-dropping body hit with a skin click; a war pattern of
  ON-ka-ON-ON rolls, heavier on the last bar of each phrase;
- a low drone on D with a slow fifth swelling in and out;
- a breathy shakuhachi-like line on the D minor pentatonic (D F G A C), with pitch bends
  into notes and vibrato on held ones;
- iron hits (a metallic inharmonic clang) on phrase starts.
Deterministic (fixed seed). Run: python3 tools/synth_boss_theme.py
"""
import numpy as np
import wave
import os

SR = 22050
BPM = 96.0
BEAT = 60.0 / BPM
BARS = 12
LENGTH = BARS * 4 * BEAT
N = int(LENGTH * SR)
rng = np.random.default_rng(7)
t_all = np.arange(N) / SR
out = np.zeros(N)


def place(sig, at):
    """Add `sig` at time `at` (s), wrapping round the loop so it stays seamless."""
    i = int(at * SR) % N
    n = len(sig)
    end = i + n
    if end <= N:
        out[i:end] += sig
    else:
        out[i:] += sig[: N - i]
        out[: end - N] += sig[N - i:]


def taiko(power=1.0, pitch=58.0):
    d = 0.9
    t = np.arange(int(d * SR)) / SR
    f = pitch * (1.0 + 1.6 * np.exp(-t * 28.0))
    phase = 2 * np.pi * np.cumsum(f) / SR
    body = np.sin(phase) * np.exp(-t * 5.0)
    click = rng.standard_normal(len(t)) * np.exp(-t * 90.0) * 0.35
    return power * (body + click) * 0.9


def ka(power=0.4):
    d = 0.12
    t = np.arange(int(d * SR)) / SR
    return power * (rng.standard_normal(len(t)) * np.exp(-t * 60.0) + np.sin(2 * np.pi * 900 * t) * np.exp(-t * 50.0) * 0.5)


def clang(power=0.35):
    d = 2.2
    t = np.arange(int(d * SR)) / SR
    parts = [(311.0, 1.0), (523.0, 0.7), (847.0, 0.5), (1193.0, 0.35), (1621.0, 0.2)]
    s = sum(a * np.sin(2 * np.pi * f * t + rng.uniform(0, 6.28)) * np.exp(-t * (1.5 + f / 900.0)) for f, a in parts)
    return power * s * 0.5


def flute(freq, dur, power=0.22):
    t = np.arange(int(dur * SR)) / SR
    bend = 1.0 - 0.06 * np.exp(-t * 14.0)  # slide up into the note
    vib = 1.0 + 0.008 * np.sin(2 * np.pi * 5.2 * t) * np.clip((t - 0.25) / 0.3, 0, 1)
    f = freq * bend * vib
    phase = 2 * np.pi * np.cumsum(f) / SR
    tone = np.sin(phase) + 0.25 * np.sin(2 * phase) + 0.08 * np.sin(3 * phase)
    breath = rng.standard_normal(len(t)) * 0.18
    # Breath through a soft low-pass (moving average).
    k = 6
    breath = np.convolve(breath, np.ones(k) / k, mode="same")
    env = np.clip(t / 0.08, 0, 1) * np.clip((dur - t) / 0.25, 0, 1)
    return power * (tone + breath) * env


# Drone: D2 with a swelling fifth.
d2 = 73.42
swell = 0.5 - 0.5 * np.cos(2 * np.pi * t_all / LENGTH * 2)
out += 0.16 * np.sin(2 * np.pi * d2 * t_all) + 0.07 * np.sin(2 * np.pi * d2 * 2 * t_all)
out += 0.06 * swell * np.sin(2 * np.pi * d2 * 1.5 * t_all)

# Drums: 4-bar phrases.
for bar in range(BARS):
    b0 = bar * 4 * BEAT
    last = bar % 4 == 3
    place(taiko(1.0), b0)
    place(ka(), b0 + 0.75 * BEAT)
    place(taiko(0.7), b0 + 1.5 * BEAT)
    place(taiko(0.85), b0 + 2.0 * BEAT)
    place(ka(), b0 + 3.0 * BEAT)
    if last:
        for k in range(4):
            place(taiko(0.6 + 0.12 * k, 64.0), b0 + 3.0 * BEAT + k * BEAT / 4)
    if bar % 4 == 0:
        place(clang(), b0)

# Flute: D minor pentatonic phrases over bars 2-3, 6-7, 10-11 (rests elsewhere).
D4 = 293.66
scale = {"D": 1.0, "F": 2 ** (3 / 12), "G": 2 ** (5 / 12), "A": 2 ** (7 / 12), "C": 2 ** (10 / 12), "D'": 2.0}
phrases = [
    [("A", 1.5), ("G", 0.5), ("F", 1.0), ("D", 3.0), ("F", 1.0), ("G", 1.0)],
    [("A", 1.0), ("C", 1.0), ("D'", 2.0), ("C", 1.0), ("A", 1.0), ("G", 2.0)],
    [("F", 1.0), ("G", 1.0), ("A", 2.0), ("G", 1.0), ("F", 1.0), ("D", 2.0)],
]
for p, phrase in enumerate(phrases):
    at = (p * 4 + 1) * 4 * BEAT
    for note, beats in phrase:
        place(flute(D4 * scale[note], beats * BEAT * 0.98), at)
        at += beats * BEAT

# Gentle room: a few early reflections (wrapping, so the loop stays seamless).
wet = np.zeros(N)
for delay, gain in [(0.043, 0.25), (0.071, 0.18), (0.113, 0.12), (0.167, 0.08)]:
    wet += np.roll(out, int(delay * SR)) * gain
out += wet

out /= np.max(np.abs(out)) * 1.15
pcm = (out * 32767).astype(np.int16)
path = os.path.join(os.path.dirname(__file__), "..", "assets", "audio", "synth", "boss_thoral.wav")
with wave.open(path, "wb") as w:
    w.setnchannels(1)
    w.setsampwidth(2)
    w.setframerate(SR)
    w.writeframes(pcm.tobytes())
print(f"wrote {os.path.normpath(path)}: {LENGTH:.1f} s")
