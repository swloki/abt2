"""找出剩余未修复的 search_icon("w-4 h-4") 调用及其上下文。"""
import re
from pathlib import Path

PAGES_DIR = Path(r"E:\work\abt\abt-web\src\pages")

lines_total = 0
for rs in sorted(PAGES_DIR.glob("*.rs")):
    lines = rs.read_text(encoding="utf-8").splitlines()
    for i, ln in enumerate(lines):
        if 'icon::search_icon("w-4 h-4")' in ln:
            lines_total += 1
            # 打印上下文：前 1 行 + 当前行
            prev = lines[i-1].strip() if i > 0 else ""
            print(f"{rs.name}:{i+1}")
            print(f"    prev: {prev}")
            print(f"    curr: {ln.strip()}")
print(f"\n剩余 search_icon('w-4 h-4') 总数: {lines_total}")
