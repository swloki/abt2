// ── Department List: client-side behavior ──

function activateTreeItem(el) {
  any('.tree-item.active').forEach(function(item) {
    item.classList.remove('active');
  });
  el.classList.add('active');
}

function filterTree() {
  var input = me('#searchInput');
  var kw = (input.value || '').toLowerCase();
  var items = any('.tree-item');
  var count = 0;
  items.forEach(function(item) {
    var name = (item.dataset.name || '').toLowerCase();
    var code = (item.dataset.code || '').toLowerCase();
    if (kw && name.indexOf(kw) === -1 && code.indexOf(kw) === -1) {
      item.style.display = 'none';
    } else {
      item.style.display = '';
      count++;
    }
  });
  var total = items.length;
  var foot = me('#treeFoot');
  if (foot) {
    foot.textContent = '共 ' + (kw ? count : total) + ' 个部门' + (kw ? '（筛选中）' : '');
  }
}

function openDeptDrawer() {
  me('#deptDrawer').classList.add('open');
  document.body.style.overflow = 'hidden';
}

function closeDeptDrawer() {
  me('#deptDrawer').classList.remove('open');
  document.body.style.overflow = '';
}

document.addEventListener('keydown', function(e) {
  if (e.key === 'Escape') closeDeptDrawer();
});
