t = open(r'E:\work\abt\static\app.css', encoding='utf-8').read()
# 简单子串搜索
for s in [
    '\\[\\&_svg\\]\\:absolute',
    '\\[\\&_svg\\]\\:left-3',
    '\\[\\&_svg\\]\\:top-1\\/2',
    '\\[\\&_svg\\]\\:-translate-y-1\\/2',
    '\\[\\&_svg\\]\\:w-4',
    '\\[\\&_svg\\]\\:h-4',
    '\\[\\&_svg\\]\\:text-muted',
]:
    print(f"{s:40} -> {'YES' if s in t else 'NO'}")
