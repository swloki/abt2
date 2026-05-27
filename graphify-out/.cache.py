import json
from graphify.cache import check_semantic_cache
from pathlib import Path

detect = json.loads(Path('graphify-out/.graphify_detect.json').read_text(encoding="utf-8"))
all_files = [f for files in detect['files'].values() for f in files]
cached_nodes, cached_edges, cached_hyperedges, uncached = check_semantic_cache(all_files)

if cached_nodes or cached_edges or cached_hyperedges:
    Path('graphify-out/.graphify_cached.json').write_text(json.dumps({'nodes': cached_nodes, 'edges': cached_edges, 'hyperedges': cached_hyperedges}, ensure_ascii=False), encoding="utf-8")
Path('graphify-out/.graphify_uncached.txt').write_text('\n'.join(uncached), encoding="utf-8")

# Count uncached by type
from collections import Counter
unc_by_ext = Counter()
for f in uncached:
    unc_by_ext[Path(f).suffix] += 1
print(f'Cache: {len(all_files)-len(uncached)} hit, {len(uncached)} need extraction')
for ext, cnt in unc_by_ext.most_common():
    print(f'  {ext}: {cnt}')