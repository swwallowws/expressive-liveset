"""Audits the file references in a Drum Rack block (or a whole gunzipped set): for each
<FileRef>, its RelativePathType, RelativePath, LivePackId and absolute Path, so a stock kit can
be checked before its absolute paths are blanked.
    python3 examples/kit_paths.py <set-or-block.xml>
Prints a tally per kind of reference, then any sample or device reference with no pack-relative
path to fall back on (those would go silent once the absolute path is blanked).
"""
import re
import sys
from collections import Counter

xml = open(sys.argv[1], encoding="utf-8").read()
refs = re.findall(r"<FileRef>(.*?)</FileRef>", xml, flags=re.S)


def val(tag, block):
    m = re.search(rf"<{tag} Value=\"([^\"]*)\"", block)
    return m.group(1) if m else None


kinds = Counter()
orphans = []
for r in refs:
    rtype, rel, pack, path = val("RelativePathType", r), val("RelativePath", r), val("LivePackId", r), val("Path", r)
    kinds[(rtype, pack or "-", "rel" if rel else "no rel", "abs" if path else "no abs")] += 1
    if path and path.lower().endswith((".wav", ".aif", ".aiff", ".adg", ".adv")) and not (rtype == "5" and rel):
        orphans.append(path)

for (rtype, pack, rel, path), n in sorted(kinds.items()):
    print(f"{n:4d}  RelativePathType {rtype}, pack {pack}, {rel}, {path}")
print(f"{len(refs)} file references; {len(orphans)} with no pack-relative path:")
for p in sorted(set(orphans)):
    print("   ", p)
