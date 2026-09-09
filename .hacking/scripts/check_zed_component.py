"""Check that the native Zed build remains a component with its API metadata."""

import sys
from pathlib import Path


def integer(data, offset):
    value = 0
    for shift in range(0, 35, 7):
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte < 0x80:
            return value, offset
    raise ValueError("Invalid WebAssembly section length")


def custom_sections(data):
    if data[:4] != b"\0asm":
        raise ValueError("Invalid WebAssembly header")
    component = data[4:8] == b"\x0d\0\x01\0"
    offset = 8
    while offset < len(data):
        kind = data[offset]
        size, start = integer(data, offset + 1)
        offset = start + size
        if offset > len(data):
            raise ValueError("Truncated WebAssembly section")
        payload = data[start:offset]
        if kind == 0:
            length, start = integer(payload, 0)
            yield payload[start : start + length], payload[start + length :]
        elif component and kind in (1, 4):
            # Core modules and nested components contain their own sections.
            yield from custom_sections(payload)


def main():
    data = Path(sys.argv[1]).read_bytes()
    if data[:8] != b"\0asm\x0d\0\x01\0":
        sys.exit(
            "Expected the Zed WASI Preview 2 component, not a host library or core module"
        )
    versions = [
        value for name, value in custom_sections(data) if name == b"zed:api-version"
    ]
    if not versions or any(len(version) != 6 for version in versions):
        sys.exit("Missing or malformed Zed extension API version metadata")
    print("Verified Zed component and API version metadata")


if __name__ == "__main__":
    main()
