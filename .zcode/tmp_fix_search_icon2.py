"""
修复第二批：剩余的标准模式变体。
用法：
    python tmp_fix_search_icon2.py          # dry-run，只预览
    python tmp_fix_search_icon2.py --apply  # 实际写回
"""
import re
import sys
from pathlib import Path

PAGES_DIR = Path(r"E:\work\abt\abt-web\src\pages")
APPLY = "--apply" in sys.argv

VARIANT = (
    "[&_svg]:absolute [&_svg]:left-3 [&_svg]:top-1/2 "
    "[&_svg]:-translate-y-1/2 [&_svg]:w-4 [&_svg]:h-4 [&_svg]:text-muted"
)

# 模式 A：单行 —— div class="relative flex-1 max-w-xs" { (icon::search_icon("w-4 h-4"))
PATTERN_INLINE = re.compile(
    r'div class="relative flex-1 max-w-xs"\s*\{\s*'
    r'\(icon::search_icon\("w-4 h-4"\)\)'
)

# 模式 B：带 style —— div class="relative flex-1 max-w-xs" style="..." {
#                                    (icon::search_icon("w-4 h-4"))
PATTERN_STYLE = re.compile(
    r'(div class="relative flex-1 max-w-xs")( style="[^"]*")(\s*\{\s*\n\s*)'
    r'\(icon::search_icon\("w-4 h-4"\)\)'
)


def fix_file(path: Path):
    text = path.read_text(encoding="utf-8")
    new_text, n1 = PATTERN_STYLE.subn(
        lambda m: f'{m.group(1)} {VARIANT}"{m.group(2)}{m.group(3)}'
                  f'(icon::search_icon(""))',
        text,
    )
    new_text, n2 = PATTERN_INLINE.subn(
        lambda m: f'div class="relative flex-1 max-w-xs {VARIANT}" {{'
                  f'(icon::search_icon(""))',
        new_text,
    )
    n = n1 + n2
    if n > 0 and APPLY:
        path.write_text(new_text, encoding="utf-8")
    return n, n1, n2


def main():
    total = 0
    for rs in sorted(PAGES_DIR.glob("*.rs")):
        if rs.name == "product_create.rs":
            continue
        n, n1, n2 = fix_file(rs)
        if n > 0:
            print(f"  {rs.name}: style={n1} inline={n2}")
            total += n
    mode = "APPLY" if APPLY else "DRY-RUN (未写回)"
    print(f"\n第二批共 {total} 处  [{mode}]")


if __name__ == "__main__":
    main()
