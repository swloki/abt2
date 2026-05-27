import json
from graphify.detect import detect
from pathlib import Path
result = detect(Path('abt-web'))
Path('graphify-out/.graphify_detect.json').write_text(json.dumps(result, ensure_ascii=False), encoding='utf-8')
print(f'{result["total_files"]} files / {result["total_words"]} words')
for k, v in result['files'].items():
    if v:
        print(f'  {k}: {len(v)} files')