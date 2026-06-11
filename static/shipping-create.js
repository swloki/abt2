// shipping-create.js — 发货申请新建页交互逻辑

// ── State ──
let selectedCustomer = null;
let selectedOrder = null;

// ── Customer Change ──
function onCustomerChange() {
  const sel = document.getElementById('shipping-customer-select');
  const customerId = sel ? sel.value : '';
  const orderInput = document.getElementById('orderPickerInput');

  if (customerId) {
    selectedCustomer = customerId;
    // Update hidden customer_id in modal search bar
    const hiddenCid = document.querySelector('#order-modal input[name="customer_id"]');
    if (hiddenCid) hiddenCid.value = customerId;
    // Enable order picker
    if (orderInput) {
      orderInput.disabled = false;
      orderInput.placeholder = '点击选择来源订单';
    }
    // Update customer info bar
    updateCustomerInfo(customerId);
    // Trigger order search refresh
    const results = document.getElementById('shipping-order-results');
    if (results) {
      htmx.trigger(results, 'intersect');
    }
  } else {
    selectedCustomer = null;
    if (orderInput) {
      orderInput.disabled = true;
      orderInput.placeholder = '请先选择客户';
      orderInput.value = '';
    }
    clearOrderData();
  }
}

function updateCustomerInfo(customerId) {
  const sel = document.getElementById('shipping-customer-select');
  const opt = sel ? sel.options[sel.selectedIndex] : null;
  if (!opt) return;
  const bar = document.getElementById('customerInfoBar');
  if (bar) bar.classList.remove('hidden-initial');
}

// ── Order Modal ──
function openOrderModal() {
  if (!selectedCustomer) return;
  const modal = document.getElementById('order-modal');
  if (modal) modal.classList.add('is-open');
  // Trigger search
  const results = document.getElementById('shipping-order-results');
  if (results) htmx.trigger(results, 'intersect');
}

// ── Select Order ──
function selectOrder(orderData) {
  selectedOrder = orderData;

  // Update order picker input
  const orderInput = document.getElementById('orderPickerInput');
  if (orderInput) orderInput.value = orderData.doc_number;

  // Update hidden order_id
  const hiddenOrderId = document.querySelector('input[name="order_id"]');
  if (hiddenOrderId) hiddenOrderId.value = orderData.id;

  // Update order detail bar
  const detail = document.getElementById('selectedOrderDetail');
  if (detail) {
    detail.classList.add('is-visible');
    const dateEl = document.getElementById('detailOrderDate');
    const statusEl = document.getElementById('detailOrderStatus');
    const amountEl = document.getElementById('detailOrderAmount');
    const productsEl = document.getElementById('detailOrderProducts');
    if (dateEl) dateEl.textContent = orderData.items && orderData.items.length > 0 ? '—' : '—';
    if (statusEl) statusEl.textContent = '—';
    if (amountEl) amountEl.textContent = '¥ ' + (orderData.total || '0');
    if (productsEl) productsEl.textContent = (orderData.items ? orderData.items.length : 0) + ' 项';
  }

  // Fill product items table
  fillItemsTable(orderData.items || []);

  // Close modal
  const modal = document.getElementById('order-modal');
  if (modal) modal.classList.remove('is-open');

  updateTotals();
}

function fillItemsTable(items) {
  const tbody = document.getElementById('lineItemsBody');
  if (!tbody) return;
  tbody.innerHTML = '';

  const warehouseSelect = document.getElementById('warehouse-default');
  const defaultWarehouse = warehouseSelect ? warehouseSelect.value : '';

  items.forEach(function(item, idx) {
    const tr = document.createElement('tr');
    tr.innerHTML =
      '<td class="line-num">' + (idx + 1) + '</td>' +
      '<td class="mono">' + (item.product_code || '') + '</td>' +
      '<td>' + (item.product_name || '') + '</td>' +
      '<td>' + (item.specification || '') + '</td>' +
      '<td>' + (item.unit || '') + '</td>' +
      '<td class="num-right">' + (item.ordered_qty || '0') + '</td>' +
      '<td class="num-right">' + (item.shipped_qty || '0') + '</td>' +
      '<td><input class="form-input num-input" type="number" name="requested_qty" min="1" placeholder="0" data-idx="' + idx + '" oninput="updateTotals()"></td>' +
      '<td><select class="form-select" name="warehouse_id" style="width:120px;padding:5px 8px;font-size:13px">' + getWarehouseOptions(defaultWarehouse) + '</select></td>' +
      '<td><button type="button" class="btn-remove-row" title="删除行" onclick="this.closest(\'tr\').remove();updateTotals()">' +
        '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>' +
      '</button></td>' +
      '<input type="hidden" name="order_item_id" value="' + item.order_item_id + '">';
    tbody.appendChild(tr);
  });
}

function getWarehouseOptions(defaultId) {
  const sel = document.getElementById('warehouse-default');
  if (!sel) return '<option value="">请选择</option>';
  let opts = '';
  for (let i = 0; i < sel.options.length; i++) {
    const selected = sel.options[i].value === defaultId ? ' selected' : '';
    opts += '<option value="' + sel.options[i].value + '"' + selected + '>' + sel.options[i].text + '</option>';
  }
  return opts;
}

function updateTotals() {
  const tbody = document.getElementById('lineItemsBody');
  if (!tbody) return;
  const rows = tbody.querySelectorAll('tr');
  let totalQty = 0;
  rows.forEach(function(row) {
    const qtyInput = row.querySelector('input[name="requested_qty"]');
    if (qtyInput && qtyInput.value) {
      totalQty += parseFloat(qtyInput.value) || 0;
    }
  });
  const totalItems = document.getElementById('totalItems');
  const totalQtyEl = document.getElementById('totalQty');
  if (totalItems) totalItems.textContent = rows.length + ' 项';
  if (totalQtyEl) totalQtyEl.textContent = totalQty;
}

// ── Clear Order ──
function clearOrder(event) {
  if (event) event.stopPropagation();
  clearOrderData();
}

function clearOrderData() {
  selectedOrder = null;
  const orderInput = document.getElementById('orderPickerInput');
  if (orderInput) orderInput.value = '';
  const hiddenOrderId = document.querySelector('input[name="order_id"]');
  if (hiddenOrderId) hiddenOrderId.value = '';
  const detail = document.getElementById('selectedOrderDetail');
  if (detail) detail.classList.remove('is-visible');
  const tbody = document.getElementById('lineItemsBody');
  if (tbody) tbody.innerHTML = '';
  updateTotals();
}

// ── Add Empty Row ──
function addRow() {
  const tbody = document.getElementById('lineItemsBody');
  if (!tbody) return;
  const idx = tbody.querySelectorAll('tr').length + 1;
  const warehouseSelect = document.getElementById('warehouse-default');
  const defaultWarehouse = warehouseSelect ? warehouseSelect.value : '';
  const tr = document.createElement('tr');
  tr.innerHTML =
    '<td class="line-num">' + idx + '</td>' +
    '<td class="mono"></td>' +
    '<td></td>' +
    '<td></td>' +
    '<td></td>' +
    '<td class="num-right">0</td>' +
    '<td class="num-right">0</td>' +
    '<td><input class="form-input num-input" type="number" name="requested_qty" min="1" placeholder="0" oninput="updateTotals()"></td>' +
    '<td><select class="form-select" name="warehouse_id" style="width:120px;padding:5px 8px;font-size:13px">' + getWarehouseOptions(defaultWarehouse) + '</select></td>' +
    '<td><button type="button" class="btn-remove-row" title="删除行" onclick="this.closest(\'tr\').remove();updateTotals()">' +
      '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>' +
    '</button></td>' +
    '<input type="hidden" name="order_item_id" value="0">';
  tbody.appendChild(tr);
  updateTotals();
}

// ── Collect Items & Submit ──
function collectItems() {
  const tbody = document.getElementById('lineItemsBody');
  if (!tbody) return;
  const items = [];
  tbody.querySelectorAll('tr').forEach(function(row) {
    const orderItemId = row.querySelector('input[name="order_item_id"]');
    const warehouseId = row.querySelector('select[name="warehouse_id"]');
    const requestedQty = row.querySelector('input[name="requested_qty"]');
    if (orderItemId && warehouseId && requestedQty && requestedQty.value) {
      items.push({
        order_item_id: parseInt(orderItemId.value) || 0,
        warehouse_id: parseInt(warehouseId.value) || 0,
        requested_qty: requestedQty.value
      });
    }
  });
  const hiddenItems = document.querySelector('form input[name="items_json"]');
  if (hiddenItems) hiddenItems.value = JSON.stringify(items);
}


function handleSave() {
  collectItems();

  // 校验：至少有一条明细行且填写了发货数量
  var tbody = document.getElementById('lineItemsBody');
  var rows = tbody ? tbody.querySelectorAll('tr') : [];
  var hasValidItem = false;
  rows.forEach(function(row) {
    var qtyInput = row.querySelector('input[name="requested_qty"]');
    if (qtyInput && qtyInput.value && parseFloat(qtyInput.value) > 0) {
      hasValidItem = true;
    }
  });

  if (!hasValidItem) {
    show_error_toast('请至少添加一条发货产品明细');
    return;
  }

  var form = document.getElementById('shipping-form');
  if (!form) return;

  // 添加/设置 draft_id（编辑已有记录时存在）
  var app = document.getElementById('shipping-app');
  var draftId = app ? app.getAttribute('data-draft-id') : null;
  var existingDraftInput = form.querySelector('input[name="draft_id"]');
  if (!existingDraftInput) {
    var draftInput = document.createElement('input');
    draftInput.type = 'hidden';
    draftInput.name = 'draft_id';
    draftInput.value = draftId || '';
    form.appendChild(draftInput);
  } else {
    existingDraftInput.value = draftId || '';
  }

  htmx.ajax('POST', '/admin/shipping/draft', { source: form, swap: 'none' });
}

// ── Restore items from draft data (called by edit page) ──
function fillItemsFromDraft(items) {
  var tbody = document.getElementById('lineItemsBody');
  if (!tbody) return;
  tbody.innerHTML = '';

  var warehouseSelect = document.getElementById('warehouse-default');
  var defaultWarehouse = warehouseSelect ? warehouseSelect.value : '';

  items.forEach(function(item, idx) {
    var tr = document.createElement('tr');
    tr.innerHTML =
      '<td class="line-num">' + (idx + 1) + '</td>' +
      '<td class="mono"></td>' +
      '<td></td>' +
      '<td>' + (item.description || '') + '</td>' +
      '<td></td>' +
      '<td class="num-right">0</td>' +
      '<td class="num-right">0</td>' +
      '<td><input class="form-input num-input" type="number" name="requested_qty" min="1" value="' + (item.requested_qty || '') + '" placeholder="0" oninput="updateTotals()"></td>' +
      '<td><select class="form-select" name="warehouse_id" style="width:120px;padding:5px 8px;font-size:13px">' + getWarehouseOptions(item.warehouse_id || defaultWarehouse) + '</select></td>' +
      '<td><button type="button" class="btn-remove-row" title="删除行" onclick="this.closest(\'tr\').remove();updateTotals()">' +
        '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>' +
      '</button></td>' +
      '<input type="hidden" name="order_item_id" value="' + (item.order_item_id || 0) + '">';
    tbody.appendChild(tr);
  });
}
