"""Extract CUE4Parse's native DLL from an official FModel .NET single-file release.

Usage: python extract_native.py FModel.exe OUTPUT_DIRECTORY
Reads the .NET bundle manifest; never executes FModel.
"""
import argparse
import io
from pathlib import Path
import struct
import zlib

SIGNATURE = bytes.fromhex("8b1202b96a612038727b930214d7a03213f5b9e6efae3318ee3b2dce24b36aae")


def string(stream):
    size, shift = 0, 0
    while True:
        b = stream.read(1)[0]
        size |= (b & 127) << shift
        if b < 128:
            return stream.read(size).decode("utf-8")
        shift += 7
        if shift > 28:
            raise ValueError("invalid bundle string")


def extract(source, output):
    data = Path(source).read_bytes()
    position = data.find(SIGNATURE)
    if position < 8:
        raise ValueError("not a .NET single-file bundle")
    offset = struct.unpack_from("<q", data, position - 8)[0]
    stream = io.BytesIO(data)
    stream.seek(offset)
    major, minor, count = struct.unpack("<iii", stream.read(12))
    if major != 6:
        raise ValueError(f"unsupported bundle format {major}.{minor}")
    string(stream)
    stream.read(40)  # deps/runtimeconfig locations and bundle flags
    found = []
    for _ in range(count):
        offset, size, compressed = struct.unpack("<qqq", stream.read(24))
        stream.read(1)
        name = string(stream)
        if Path(name).name.lower() != "cue4parse-natives.dll":
            continue
        content = data[offset:offset + (compressed or size)]
        if compressed:
            content = zlib.decompress(content, -15)
        if len(content) != size:
            raise ValueError("native DLL size mismatch")
        target = Path(output) / "CUE4Parse-Natives.dll"
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.exists() and target.read_bytes() != content:
            raise FileExistsError(target)
        target.write_bytes(content)
        found.append(str(target))
    if not found:
        raise ValueError("CUE4Parse-Natives.dll absent from bundle")
    return found


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source")
    parser.add_argument("output")
    args = parser.parse_args()
    for path in extract(args.source, args.output):
        print(path)
