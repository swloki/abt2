"""
批量修复搜索框 SVG 换行 bug（标准模式）。

标准模式（错误）：
    div class="relative flex-1 max-w-xs" {
        (icon::search_icon("w-4 h-4"))
        input class="w-full pl-9 ..."

修复后：
    div class="relative flex-1 max-w-xs [&_svg]:absolute ... " {
        (icon::search_icon(""))
        input ...

变体文件（外层 div 非标准 class）由人工单独处理，跳过。
"""
import re
from pathlib import Path

PAGES_DIR = Path(r"E:\work\abt\abt-web\src\pages")
SKIP_FILES = {"category_list.rs", "department_list.rs", "product_create.rs"}

VARIANT = (
    "[&_svg]:absolute [&_svg]:left-3 [&_svg]:top-1/2 "
    "[&_svg]:-translate-y-1/2 [&_svg]:w-4 [&_svg]:h-4 [&_svg]:text-muted"
)

# 精确匹配标准模式整段（div 开标签 + 大括号 + 换行 + search_icon("w-4 h-4"))
PATTERN = re.compile(
    r'div class="relative flex-1 max-w-xs"(\s*\{\s*\n\s*)'
    r'\(icon::search_icon\("w-4 h-4"\)\)'
)


def fix_file(path: Path) -> int:
    text = path.read_text(encoding="utf-8")
    new_text, n = PATTERN.subn(
        lambda m: f'div class="relative flex-1 max-w-xs {VARIANT}"'
                  f'{m.group(1)}(icon::search_icon(""))',
        text,
    )
    if n > 0:
        path.write_text(new_text, encoding="utf-8")
    return n


def main():
    total = 0
    files_changed = 0
    for rs in sorted(PAGES_DIR.glob("*.rs")):
        if rs.name in SKIP_FILES:
            continue
        n = fix_file(rs)
        if n > 0:
            files_changed += 1
            total += n
            print(f"  {rs.name}: {n} 处")
    print(f"\n共修改 {files_changed} 个文件，{total} 处")


if __name__ == "__main__":
    main()
