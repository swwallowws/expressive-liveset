"""Makes templates/devices/drum-kit.xml from a gunzipped Live set holding a stock Core Library
kit on a MIDI track: takes its Drum Rack block and strips every machine-specific path, so the kit
plays on any install.
    python3 examples/clean_kit.py <set.xml> <out.xml>
For each <FileRef>:
  - pack-relative (RelativePathType 5, a LivePackId): keep RelativePath and the pack, blank the
    absolute Path, which names this machine's Live app; Live resolves the sample in its own
    Core Library.
  - anything else that names a path (staff preset folders left in Ableton's presets, files beside
    a project): cleared to an empty reference. These only link a preset's name; the device's
    settings are stored inline.
Refuses to write a block that still names /Users/ or /Applications/.
"""
import re
import sys

src, out = sys.argv[1], sys.argv[2]
xml = open(src, encoding="utf-8").read()
start = xml.index("<DrumGroupDevice ")
end = xml.index("</DrumGroupDevice>", start) + len("</DrumGroupDevice>")
block = xml[start:end]


def clean(m):
    ref = m.group(0)
    rtype = re.search(r'<RelativePathType Value="(\d+)"', ref)
    pack = re.search(r'<LivePackId Value="([^"]*)"', ref)
    if rtype and rtype.group(1) == "5" and pack and pack.group(1):
        return re.sub(r'<Path Value="[^"]*"', '<Path Value=""', ref)
    ref = re.sub(r'<RelativePathType Value="\d+"', '<RelativePathType Value="0"', ref)
    ref = re.sub(r'<RelativePath Value="[^"]*"', '<RelativePath Value=""', ref)
    return re.sub(r'<Path Value="[^"]*"', '<Path Value=""', ref)


block = re.sub(r"<FileRef(?: [^>]*)?>.*?</FileRef>", clean, block, flags=re.S)
# Browser paths outside FileRefs (a preset's place in someone's browser) carry no sound.
block = re.sub(r'(<BrowserContentPath Value=")[^"]*(")', r"\1\2", block)
for leak in ("/Users/", "/Applications/"):
    if leak in block:
        at = block.index(leak)
        context = block[max(0, at - 400):at + 80]
        sys.exit(f"still names {leak}; clean that kind of reference too:\n{context}")
# Live's indentation is a quarter of the block and means nothing to its reader; drop it, since the
# block is compiled into every build that writes a set.
block = "\n".join(line.lstrip("\t") for line in block.split("\n"))
open(out, "w", encoding="utf-8").write(block + "\n")
pads = block.count("<DrumBranch ")
print(f"{out}: {block.count(chr(10)) + 1} lines, {pads} pads")
