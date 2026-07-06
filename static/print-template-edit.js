// ── 打印模板编辑器：自包含加载 CodeMirror 5 + 初始化 + 变量插入 + HTMX 预览/保存/打印 ──
// 由 edit_form 的 hyperscript（on load）加载到 <head>（body 内 <script src> 在本系统不执行）。
(function () {
  var ta = document.getElementById('html-editor');
  if (!ta || window.__pteLoaded) return;
  window.__pteLoaded = true;

  function loadScript(src, cb) {
    var s = document.createElement('script');
    s.src = src;
    s.onload = cb;
    document.head.appendChild(s);
  }
  function loadCss(href) {
    var l = document.createElement('link');
    l.rel = 'stylesheet';
    l.href = href;
    document.head.appendChild(l);
  }

  function start() {
    if (typeof CodeMirror === 'undefined') return;
    // 动态创建 toast 容器（初始 HTML 渲染的在编辑页会被某机制剥，运行时 createElement 的不剥）
    if (!document.getElementById('pte-toast-area')) {
      var pteToast = document.createElement('div');
      pteToast.id = 'pte-toast-area';
      pteToast.style.cssText = 'position:fixed;top:16px;right:16px;width:360px;max-width:calc(100vw-32px);z-index:99999;display:flex;flex-direction:column;gap:8px';
      document.body.appendChild(pteToast);
    }
    var cm = CodeMirror.fromTextArea(document.getElementById('html-editor'), {
      mode: { name: 'xml', htmlMode: true },
      lineNumbers: true,
      tabSize: 2,
      indentUnit: 2,
      indentWithTabs: false,
      theme: 'material-darker',
      lineWrapping: true,
    });

    // 编辑器高度撑满 #editor-pane（与变量面板等高）
    var wrap = cm.getWrapperElement();
    wrap.style.flex = '1 1 0';
    wrap.style.minHeight = '0';
    cm.refresh();

    // Ctrl+S / Cmd+S 保存（阻止浏览器默认「保存网页」）
    document.addEventListener('keydown', function (e) {
      if ((e.ctrlKey || e.metaKey) && (e.key === 's' || e.key === 'S')) {
        e.preventDefault();
        var form = document.querySelector('form[hx-post*="/edit"]');
        if (form) htmx.trigger(form, 'submit');
      }
    });

    // 保存成功 → 拉全局 toast 系统（/api/toast 返回标准 toast HTML）显示到 #pte-toast-area。
    // 监听 form 的 afterRequest（200 + 空响应 = 保存成功）；render-preview 不走 form，不会误触发。
    var pteForm = document.querySelector('form[hx-post*="/edit"]');
    if (pteForm) {
      pteForm.addEventListener('htmx:afterRequest', function (ev) {
        var xhr = ev.detail && ev.detail.xhr;
        if (xhr && xhr.status === 200 && (xhr.responseText || '').length === 0) {
          htmx.ajax('GET', '/api/toast', { target: '#pte-toast-area', swap: 'innerHTML' });
        }
      });
    }

    // 编辑 → debounce 触发预览
    var debounceTimer;
    cm.on('change', function () {
      clearTimeout(debounceTimer);
      debounceTimer = setTimeout(updatePreview, 400);
    });

    // 变量 chip 插入（hyperscript: on click call insertPrintSnippet(my @data-snippet)）
    window.insertPrintSnippet = function (snippet) {
      cm.replaceSelection(snippet);
      cm.focus();
    };

    // 明细循环块（HTML 属性换行会被浏览器规范化为空格，故在 JS 里拼）
    var LOOP_TEMPLATES = {
      delivery_note:
        '{% for item in 明细 %}\n<tr>\n  <td>{{ loop.index }}</td>\n  <td>{{ item.产品编码 }}</td>\n  <td>{{ item.产品名称 }}</td>\n  <td>{{ item.单位 }}</td>\n  <td>{{ item.本次出库数量 }}</td>\n</tr>\n{% endfor %}',
      quotation:
        '{% for item in 明细 %}\n<tr>\n  <td>{{ item.行号 }}</td>\n  <td>{{ item.产品名称 }}</td>\n  <td>{{ item.数量 }}</td>\n  <td>{{ item.单价 }}</td>\n  <td>{{ item.金额 }}</td>\n</tr>\n{% endfor %}',
      sales_order:
        '{% for item in 明细 %}\n<tr>\n  <td>{{ item.行号 }}</td>\n  <td>{{ item.产品名称 }}</td>\n  <td>{{ item.数量 }}</td>\n  <td>{{ item.单价 }}</td>\n  <td>{{ item.金额 }}</td>\n</tr>\n{% endfor %}',
      purchase_order:
        '{% for item in 明细 %}\n<tr>\n  <td>{{ item.行号 }}</td>\n  <td>{{ item.产品名称 }}</td>\n  <td>{{ item.数量 }}</td>\n  <td>{{ item.单价 }}</td>\n  <td>{{ item.金额 }}</td>\n</tr>\n{% endfor %}',
    };
    window.insertDetailLoop = function () {
      var sel = document.querySelector('select[name="document_type"]');
      var dt = sel ? sel.value : 'delivery_note';
      cm.replaceSelection(LOOP_TEMPLATES[dt] || LOOP_TEMPLATES.delivery_note);
      cm.focus();
    };

    // 预览：htmx.ajax POST render-preview（后端 minijinja 渲染）。
    // target:'body' + swap:'none' 只拿响应、不替换 body（否则编辑器 DOM 消失）。
    window.updatePreview = function () {
      var sel = document.querySelector('select[name="document_type"]');
      htmx.ajax('POST', '/admin/system/print-templates/render-preview', {
        target: '#html-preview',
        swap: 'none',
        values: { html_content: cm.getValue(), document_type: sel ? sel.value : 'delivery_note' },
      });
    };

    // 打印：打印 iframe 里的渲染结果（送货单）。iframe 可能 display:none（源码 tab），
    // 打印前临时显示（display:none 的 iframe 浏览器不打印），打印后恢复。
    window.printTemplate = function () {
      if (window.updatePreview) window.updatePreview();
      setTimeout(function () {
        var ifr = document.getElementById('html-preview');
        if (!ifr || !ifr.contentWindow) return;
        var prev = ifr.style.display;
        ifr.style.display = 'block';
        ifr.contentWindow.focus();
        ifr.contentWindow.print();
        setTimeout(function () { ifr.style.display = prev; }, 1000);
      }, 600);
    };

    // 保存：HTMX submit 不触发 CM 自动回写，在 htmx:configRequest 把 html_content 替换为 CM 当前内容。
    document.body.addEventListener('htmx:configRequest', function (e) {
      var cfg = e.detail && e.detail.requestConfig;
      if (!cfg || !cfg.path) return;
      if (cfg.path.indexOf('print-templates') === -1) return;
      if (cfg.path.indexOf('render-preview') !== -1) return;
      if (e.detail.parameters && 'html_content' in e.detail.parameters) {
        e.detail.parameters['html_content'] = cm.getValue();
      }
    });

    // 预览响应写进 iframe
    // 双保险：form submit（capture 阶段，早于 HTMX 序列化）时 cm.save() 把 CM 内容写回 textarea。
    // 单靠 htmx:configRequest 改 parameters 在某些 HTMX 版本不可靠。
    var pteForm = document.getElementById('html-editor').form;
    if (pteForm) {
      pteForm.addEventListener('submit', function () { cm.save(); }, true);
    }

    document.body.addEventListener('htmx:afterRequest', function (e) {
      var cfg = e.detail && e.detail.requestConfig;
      if (!cfg || !cfg.path || cfg.path.indexOf('render-preview') === -1) return;
      var ifr = document.getElementById('html-preview');
      if (!ifr || !e.detail.xhr || e.detail.xhr.status !== 200) return;
      var d = ifr.contentDocument || ifr.contentWindow.document;
      d.open();
      d.write(e.detail.xhr.responseText);
      d.close();
    });

    updatePreview();
  }

  // 自包含加载 codemirror + mode/xml + 主题 CSS，加载完初始化
  loadCss('/codemirror.css');
  loadCss('/theme/material-darker.css');
  if (typeof CodeMirror !== 'undefined') {
    start();
  } else {
    loadScript('/codemirror.js', function () {
      loadScript('/mode/xml/xml.js', start);
    });
  }
})();
