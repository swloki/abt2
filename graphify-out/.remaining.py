from pathlib import Path
files = Path('graphify-out/.semantic_files.txt').read_text(encoding='utf-8').strip().split('\n')
# Already dispatched: first 50 files (chunks 1-2)
remaining = files[50:]
print(f'Remaining: {len(remaining)} files')
# Print in chunks of 25
for i in range(0, len(remaining), 25):
    chunk = remaining[i:i+25]
    chunk_num = 3 + i // 25
    print(f'\nChunk {chunk_num} ({len(chunk)} files):')
    for f in chunk:
        print(f'  {f}')