"""Call Aura's installed local MCP bridge with its bundled Python runtime.

Usage: <AuraForUnity/python/python.exe> Tools/aura_call.py TOOL [arguments.json] [capture.png]
Use TOOL=list to inspect schemas. This calls tools directly, not an Aura chat agent.
"""
import asyncio
import base64
import json
import os
from pathlib import Path
import sys

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client


def native_failure(value):
    # Some Aura tools return a JSON array containing an encoded JSON result.
    if isinstance(value, str):
        try:
            return native_failure(json.loads(value))
        except (ValueError, TypeError):
            return False
    if isinstance(value, list):
        return any(native_failure(item) for item in value)
    return isinstance(value, dict) and (value.get("Success") is False or value.get("success") is False)


async def main():
    params = StdioServerParameters(command=sys.executable, args=["-m", "aura_unity_mcp.server"])
    with open(os.devnull, "w") as errors:
        async with stdio_client(params, errlog=errors) as (reader, writer):
            async with ClientSession(reader, writer) as session:
                await session.initialize()
                if sys.argv[1] == "list":
                    result = await session.list_tools()
                    print(json.dumps([{"name": t.name, "description": t.description, "schema": t.inputSchema} for t in result.tools], ensure_ascii=True))
                    return
                project = str(Path(__file__).resolve().parent.parent)
                await session.call_tool("set_unity_project_root", {"unity_project_root": project})
                arguments = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8-sig")) if len(sys.argv) > 2 else {}
                result = await session.call_tool(sys.argv[1], arguments)
                for content in result.content:
                    if content.type == "text":
                        print(content.text)
                        try:
                            native = json.loads(content.text)
                        except (ValueError, TypeError):
                            native = None
                        if native_failure(native):
                            raise RuntimeError("Aura's native operation failed")
                    elif content.type == "image" and len(sys.argv) > 3:
                        capture = Path(sys.argv[3])
                        capture.parent.mkdir(parents=True, exist_ok=True)
                        capture.write_bytes(base64.b64decode(content.data))
                        print("Capture saved: " + str(capture))
                if result.isError:
                    raise RuntimeError("Aura MCP tool returned an error")


if __name__ == "__main__":
    asyncio.run(main())
