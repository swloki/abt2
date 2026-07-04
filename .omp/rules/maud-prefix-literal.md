---
description: "Rust 2024 Maud prefix literal 陷阱：class=\"x-y\" 紧接 style= 会编译报错，class 字符串末尾加空格"
condition: "class=\"[^\"]*-[^\" ]\"\\s+style"
scope: "tool:edit(*.rs), tool:write(*.rs)"
---

你刚写了 `class="xxx-yyy" style="..."` —— 这是 **Rust 2024 edition 的 prefix literal 陷阱**。

**原因**：Rust 2024 lexer 把 `"xxx-yyy"` 后直接跟标识符 `style` 解析为「字符串字面前缀 + 字面量」（类似 `b"..."`、`r#"..."#`），编译报错：

```
error: prefix `yyy` is unknown
```

**解法**：class 字符串末尾加一个空格，破坏 `<value>-<identifier>` 的相邻关系：

```rust
// ❌ 错误：触发 prefix literal
div class="cascade-product" style="..." { ... }

// ✅ 正确：末尾空格断开
div class="cascade-product " style="..." { ... }
//                       ^ 注意这个空格
```

**判别**：只在 `"<value>-<identifier>"` 紧跟下一属性时触发。`class="x-y"` 后跟 `>` 或其他不在意前缀的 token 不会触发。但 Maud 里 `class` 后几乎必然有别的属性，养成习惯：**所有带连字符的 class 字符串末尾一律加空格**。

依据：`AGENTS.md` "Rust 2024 edition — Maud prefix literal 陷阱"、`abt-web/CLAUDE.md` Constraints 第 1 条。
