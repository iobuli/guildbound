"""Kev reference-file recording and validation."""
import json
import math
from typing import Mapping, Optional


class ReferenceRecorder:
    def __init__(self, name: str, height: float = 1.75, floor: float = 0.0,
                 source: Optional[Mapping] = None):
        self.name = name
        self.height = float(height)
        self.floor = float(floor)
        self.source = dict(source or {})
        self.samples = []

    def add(self, frame: Optional[Mapping]):
        if not frame:
            return
        landmarks = frame.get("landmarks", {})
        clean = {}
        for name, value in landmarks.items():
            xyz = [float(x) for x in value]
            if len(xyz) == 3 and all(math.isfinite(x) for x in xyz):
                clean[name] = xyz
        if "pelvis" not in clean:
            return
        t = float(frame["t"])
        if self.samples and t <= self.samples[-1]["t"]:
            return
        self.samples.append({"t": round(t, 6), "landmarks": clean,
                             "confidence": dict(frame.get("confidence", {}))})

    def to_dict(self) -> dict:
        duration = self.samples[-1]["t"] if self.samples else 0.0
        return {"name": self.name, "model": "autoanim", "clip": self.name,
                "height": self.height, "floor": self.floor, "duration": duration,
                "source": self.source, "samples": self.samples}

    def validate(self):
        if len(self.samples) < 2:
            raise ValueError("reference needs at least two valid frames")
        times = [s["t"] for s in self.samples]
        if any(b <= a for a, b in zip(times, times[1:])):
            raise ValueError("reference timestamps must increase")
        return {"frames": len(self.samples), "duration": times[-1],
                "landmarks": len(set().union(*(s["landmarks"] for s in self.samples)))}

    def write(self, path: str):
        self.validate()
        with open(path, "w", encoding="utf-8") as f:
            json.dump(self.to_dict(), f, indent=1)
            f.write("\n")
