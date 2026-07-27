"""One-off helper: turn the JS arrays in ../programs.txt into binary .ch8 files."""

import re
from pathlib import Path

SRC = Path(__file__).resolve().parent.parent / "programs.txt"
OUT = Path(__file__).resolve().parent / "roms"

FILE_NAMES = {
    "ibm": "ibm_logo",
    "guess": "guess",
    "computer": "computer",
    "collide": "collide",
    "branch": "branch",
    "compare": "compare",
    "loop": "loop",
    "mirrir": "mirror",
    "stack1": "stack1",
    "stack2": "stack2",
    "next": "next",
    "verticalClip": "vertical_clip",
    "spaceInvaders": "space_invaders",
    "pong": "pong",
}

text = SRC.read_text(encoding="utf-8")
OUT.mkdir(exist_ok=True)

pattern = re.compile(r"const\s+(\w+)\s*=\s*\[(.*?)\];", re.DOTALL)
found = {}

for match in pattern.finditer(text):
    name = match.group(1)
    values = [int(v, 0) for v in match.group(2).replace("\n", " ").split(",") if v.strip()]
    assert all(0 <= v <= 255 for v in values), name
    found[name] = bytes(values)

for name, data in found.items():
    file_name = FILE_NAMES.get(name)
    assert file_name, f"unmapped program: {name}"
    (OUT / f"{file_name}.ch8").write_bytes(data)
    print(f"{name:>14} -> {file_name}.ch8 ({len(data)} bytes)")

missing = set(FILE_NAMES) - set(found)
assert not missing, f"missing: {missing}"
