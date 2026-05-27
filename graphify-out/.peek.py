import json
from pathlib import Path
from collections import Counter
detect = json.loads(Path('graphify-out/.graphify_detect.json').read_text(encoding="utf-8"))
code_dirs = Counter()
doc_dirs = Counter()
for f in detect['files'].get('code', []):
    p = str(f).replace("E:\\work\\abt\\abt-web\\", "")
    parts = p.split("\\")
    code_dirs[parts[0] if len(parts) > 1 else p] += 1
for f in detect['files'].get('document', []):
    p = str(f).replace("E:\\work\\abt\\abt-web\\", "")
    parts = p.split("\\")
    doc_dirs[parts[0] if len(parts) > 1 else p] += 1
print("CODE by top dir:")
for d, c in code_dirs.most_common(20):
    print(f"  {c:4d}  {d}")
print("\nDOC by top dir:")
for d, c in doc_dirs.most_common(10):
    print(f"  {c:4d}  {d}")