"""Command-line entry point for Kev AutoAnim."""
import argparse
import json
import math
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
if HERE not in sys.path:
    sys.path.insert(0, HERE)

from client import KevClient
from core import MotionConfig, MotionPipeline, synthetic_frames
from interactions import align_files
from movesets import compile_file
from reference import ReferenceRecorder
from styles import load_styles, style_file


def read_jsonl(path):
    source = sys.stdin if path == "-" else open(path, encoding="utf-8")
    try:
        for number, line in enumerate(source, 1):
            line = line.strip()
            if not line:
                continue
            try:
                yield json.loads(line)
            except json.JSONDecodeError as exc:
                raise SystemExit(f"{path}:{number}: {exc}") from exc
    finally:
        if source is not sys.stdin:
            source.close()


def config_of(args):
    return MotionConfig(input_space=args.space, target_height_m=args.height,
                        preserve_metric_scale=args.preserve_scale, mirror_x=args.mirror,
                        min_confidence=args.min_confidence)


def add_motion_args(parser):
    parser.add_argument("--space", choices=("normalized_image", "image_depth", "camera_m", "kev"),
                        default="normalized_image")
    parser.add_argument("--height", type=float, default=1.75, help="target character height in metres")
    parser.add_argument("--preserve-scale", action="store_true", help="keep metric tracker scale")
    parser.add_argument("--mirror", action="store_true", help="flip camera X")
    parser.add_argument("--min-confidence", type=float, default=0.25)


def process_frames(args, frames, stream=False):
    pipeline = MotionPipeline(config_of(args))
    recorder = ReferenceRecorder(args.name, args.height, source={"tool": "kev-autoanim", "space": args.space})
    client = KevClient(args.url, args.token) if stream else None
    start_wall = time.perf_counter()
    first_t = None
    sent = 0
    last_response = None
    for raw in frames:
        frame = pipeline.process(raw)
        if not frame:
            continue
        recorder.add(frame)
        if client:
            if args.realtime:
                first_t = frame["t"] if first_t is None else first_t
                due = start_wall + (frame["t"] - first_t) / max(args.rate, 1e-6)
                time.sleep(max(0.0, due - time.perf_counter()))
            last_response = client.send(frame, args.inplace)
            sent += 1
    if args.output:
        recorder.write(args.output)
    result = {**pipeline.stats(), "recorded": len(recorder.samples), "sent": sent,
              "output": os.path.abspath(args.output) if args.output else None}
    if last_response:
        result["last_reach_error_m"] = last_response.get("reach_error_m", {})
        result["last_balance"] = last_response.get("balance", {})
    print(json.dumps(result, indent=2))
    return result


def command_process(args):
    process_frames(args, read_jsonl(args.input), False)


def command_stream(args):
    process_frames(args, read_jsonl(args.input), True)


def command_simulate(args):
    frames = synthetic_frames(int(round(args.seconds * args.fps)), args.fps, args.noise,
                              args.dropout, args.spikes, args.seed)
    with open(args.output, "w", encoding="utf-8") as f:
        for frame in frames:
            f.write(json.dumps(frame, separators=(",", ":")) + "\n")
    print(json.dumps({"frames": int(round(args.seconds * args.fps)), "output": os.path.abspath(args.output)}, indent=2))


def command_limit(args):
    pipeline = MotionPipeline(MotionConfig(target_height_m=args.height))
    positions = []
    nonfinite = 0
    t0 = time.perf_counter()
    for raw in synthetic_frames(args.frames, args.fps, args.noise, args.dropout, args.spikes, args.seed):
        frame = pipeline.process(raw)
        if not frame:
            continue
        flat = [x for p in frame["landmarks"].values() for x in p]
        nonfinite += sum(not math.isfinite(x) for x in flat)
        if "ball_l" in frame["landmarks"]:
            positions.append(frame["landmarks"]["ball_l"])
    elapsed = time.perf_counter() - t0
    speeds = []
    for a, b in zip(positions, positions[1:]):
        speeds.append(math.sqrt(sum((x - y) ** 2 for x, y in zip(a, b))) * args.fps)
    speeds.sort()
    p99 = speeds[min(int(len(speeds) * 0.99), len(speeds) - 1)] if speeds else 0.0
    result = {**pipeline.stats(), "seconds": round(elapsed, 3),
              "frames_per_second": round(args.frames / max(elapsed, 1e-9)),
              "nonfinite_values": nonfinite, "left_foot_speed_p99_m_s": round(p99, 4),
              "input_dropout_rate": args.dropout, "input_spike_rate": args.spikes}
    result["passed"] = nonfinite == 0 and result["frames_out"] >= int(args.frames * 0.95)
    print(json.dumps(result, indent=2))
    if not result["passed"]:
        raise SystemExit(1)


def command_style(args):
    out = style_file(args.input, args.output, args.style, args.strength, args.rank, args.styles)
    print(json.dumps({"name": out["name"], "duration": out["duration"], "frames": len(out["samples"]),
                      "output": os.path.abspath(args.output)}, indent=2))


def command_moveset(args):
    styles = load_styles(args.styles)
    if args.style not in styles:
        raise SystemExit(f"unknown style {args.style!r}; choose from {', '.join(sorted(styles))}")
    links = {}
    for value in args.link:
        if "=" not in value:
            raise SystemExit(f"--link expects OLD=NEW, got {value!r}")
        old, new = value.split("=", 1)
        links[old] = new
    out, manifest = compile_file(args.input, args.output, args.manifest, args.style, styles[args.style],
                                 args.strength, args.rank, links, args.name)
    print(json.dumps({"moveset": out["name"], "attacks": len(out.get("light", [])) + len(out.get("heavy", [])),
                      "output": os.path.abspath(args.output), "manifest": os.path.abspath(args.manifest)}, indent=2))


def command_align(args):
    attacker_actor = {"model": args.attacker_model, "clip": args.attacker_clip} if args.attacker_model else None
    defender_actor = {"model": args.defender_model, "clip": args.defender_clip} if args.defender_model else None
    _aligned, manifest = align_files(args.attacker, args.defender, args.contacts, args.output, args.manifest,
                                     args.attacker_time, args.defender_time, attacker_actor, defender_actor)
    if args.preview:
        actor = manifest.get("defender", {})
        transform = manifest["viewer_enemy_transform"]
        if not actor.get("model"):
            raise SystemExit("--preview needs --defender-model (and usually --defender-clip)")
        KevClient(args.url, args.token).command({"cmd": "onion", "model": actor["model"],
                                                 "clip": actor.get("clip"), "position": transform["position"],
                                                 "yaw_deg": transform["yaw_deg"], "opacity": args.opacity,
                                                 "time": args.defender_time})
    print(json.dumps({"alignment_error": manifest["alignment_error"],
                      "aligned": os.path.abspath(args.output), "manifest": os.path.abspath(args.manifest),
                      "previewed": bool(args.preview)}, indent=2))


def parser():
    ap = argparse.ArgumentParser(description="AI-pose/depth bridge for Kev Studio")
    sub = ap.add_subparsers(dest="command", required=True)

    sim = sub.add_parser("simulate", help="write deterministic noisy tracker JSONL")
    sim.add_argument("output")
    sim.add_argument("--seconds", type=float, default=3.0)
    sim.add_argument("--fps", type=float, default=60.0)
    sim.add_argument("--noise", type=float, default=0.003)
    sim.add_argument("--dropout", type=float, default=0.05)
    sim.add_argument("--spikes", type=float, default=0.005)
    sim.add_argument("--seed", type=int, default=7)
    sim.set_defaults(func=command_simulate)

    proc = sub.add_parser("process", help="convert tracker JSONL to a Kev reference")
    proc.add_argument("input", help="JSONL path, or - for stdin")
    proc.add_argument("output")
    proc.add_argument("--name", default="autoanim_capture")
    add_motion_args(proc)
    proc.set_defaults(func=command_process)

    stream = sub.add_parser("stream", help="process JSONL and drive a running Kev Studio")
    stream.add_argument("input", help="JSONL path, or - for stdin")
    stream.add_argument("--output", help="also record a Kev reference JSON")
    stream.add_argument("--name", default="autoanim_live")
    stream.add_argument("--url", default="http://127.0.0.1:8765")
    stream.add_argument("--token", default="")
    stream.add_argument("--realtime", action="store_true")
    stream.add_argument("--rate", type=float, default=1.0)
    stream.add_argument("--inplace", action="store_true")
    add_motion_args(stream)
    stream.set_defaults(func=command_stream)

    limit = sub.add_parser("limit-test", help="stress filter with noise, dropouts and depth spikes")
    limit.add_argument("--frames", type=int, default=10000)
    limit.add_argument("--fps", type=float, default=60.0)
    limit.add_argument("--height", type=float, default=1.75)
    limit.add_argument("--noise", type=float, default=0.006)
    limit.add_argument("--dropout", type=float, default=0.15)
    limit.add_argument("--spikes", type=float, default=0.01)
    limit.add_argument("--seed", type=int, default=19)
    limit.set_defaults(func=command_limit)

    style = sub.add_parser("style-reference", help="make a regional/strength/rank motion variant")
    style.add_argument("input")
    style.add_argument("output")
    style.add_argument("--style", required=True)
    style.add_argument("--styles", default=os.path.join(HERE, "styles.json"))
    style.add_argument("--strength", type=float, default=0.5)
    style.add_argument("--rank", type=float, default=0.5)
    style.set_defaults(func=command_style)

    moveset = sub.add_parser("moveset", help="compile a game moveset for one region/strength/rank")
    moveset.add_argument("input")
    moveset.add_argument("output")
    moveset.add_argument("--manifest", required=True)
    moveset.add_argument("--style", required=True)
    moveset.add_argument("--styles", default=os.path.join(HERE, "styles.json"))
    moveset.add_argument("--strength", type=float, default=0.5)
    moveset.add_argument("--rank", type=float, default=0.5)
    moveset.add_argument("--name")
    moveset.add_argument("--link", action="append", default=[], metavar="OLD=NEW")
    moveset.set_defaults(func=command_moveset)

    align = sub.add_parser("align", help="align paired references at named contact points")
    align.add_argument("attacker")
    align.add_argument("defender")
    align.add_argument("contacts")
    align.add_argument("output", help="aligned attacker reference")
    align.add_argument("--manifest", required=True)
    align.add_argument("--attacker-time", type=float, required=True)
    align.add_argument("--defender-time", type=float, required=True)
    align.add_argument("--attacker-model")
    align.add_argument("--attacker-clip")
    align.add_argument("--defender-model")
    align.add_argument("--defender-clip")
    align.add_argument("--preview", action="store_true", help="send enemy onion command to Kev Studio")
    align.add_argument("--opacity", type=float, default=0.28)
    align.add_argument("--url", default="http://127.0.0.1:8765")
    align.add_argument("--token", default="")
    align.set_defaults(func=command_align)
    return ap


def main():
    args = parser().parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
