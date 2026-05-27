from pathlib import Path
lines = Path('graphify-out/.graphify_uncached.txt').read_text(encoding='utf-8').strip().split('\n')
doc_exts = {'.md', '.txt', '.html'}
img_exts = {'.png', '.jpg', '.jpeg', '.webp', '.svg'}
docs = [f for f in lines if Path(f).suffix.lower() in doc_exts]
imgs = [f for f in lines if Path(f).suffix.lower() in img_exts]
print(f'Docs: {len(docs)}, Images: {len(imgs)}')
# Save just the semantic-needing files
Path('graphify-out/.semantic_files.txt').write_text('\n'.join(docs + imgs), encoding='utf-8')