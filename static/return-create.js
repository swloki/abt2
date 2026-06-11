// Purchase Return Create — client-side logic
// Loaded by purchase_return_create.rs

(function () {
  // ── HTMX afterSwap: process order data ──
  document.body.addEventListener("htmx:afterSettle", function (e) {
    var target = e.detail?.target;
    if (!target || target.id !== "pr-order-data") return;

    var container = target.querySelector("[data-supplier-name]");
    if (!container) return;

    // Populate supplier info
    var supplierName = container.getAttribute("data-supplier-name") || "—";
    var contact = container.getAttribute("data-contact") || "—";
    var phone = container.getAttribute("data-phone") || "—";

    var nameInput = document.getElementById("pr-supplier-name");
    var contactInput = document.getElementById("pr-contact");
    var phoneInput = document.getElementById("pr-phone");
    if (nameInput) nameInput.value = supplierName;
    if (contactInput) contactInput.value = contact;
    if (phoneInput) phoneInput.value = phone;

    // order_id is submitted directly from the select[name="order_id"]

    // Build item rows
    var itemDivs = container.querySelectorAll("[data-item]");
    var tbody = document.getElementById("pr-item-tbody");
    if (!tbody) return;
    tbody.innerHTML = "";

    itemDivs.forEach(function (div, idx) {
      var data = JSON.parse(div.getAttribute("data-item"));
      var lineNo = idx + 1;
      var returnedQty = data.returned_qty || data.order_qty || "0";
      var unitPrice = data.unit_price || "0";
      var subtotal = (parseFloat(returnedQty) * parseFloat(unitPrice)).toFixed(2);

      var tr = document.createElement("tr");
      tr.innerHTML =
        '<td class="line-num">' + lineNo + "</td>" +
        '<td class="mono">' + esc(data.product_code) + "</td>" +
        "<td>" + esc(data.product_name) + "</td>" +
        "<td>" + esc(data.specification) + "</td>" +
        "<td>" + esc(data.unit) + "</td>" +
        '<td class="num-right">' + fmtNum(data.order_qty) + "</td>" +
        '<td class="num-right">' + fmtNum(data.received_qty) + "</td>" +
        '<td><input class="form-input num-input" type="number" step="0.01" min="0" ' +
          'style="width:110px;text-align:right;padding:5px 8px;font-size:13px;font-family:var(--font-mono);border:1px solid var(--border);border-radius:var(--radius-sm)" ' +
          'name="returned_qty" value="' + returnedQty + '" data-idx="' + idx + '"></td>' +
        '<td class="num-right mono">' + fmtNum(unitPrice) + "</td>" +
        '<td class="num-right mono line-subtotal" data-idx="' + idx + '">' + subtotal + "</td>" +
        '<td><button type="button" class="btn-remove-row" title="删除行" onclick="this.closest(\'tr\').remove();PRCreate.recalcLineNos()">' +
          '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg></button></td>' +
        '<input type="hidden" name="order_item_id" value="' + data.order_item_id + '">' +
        '<input type="hidden" name="product_id" value="' + data.product_id + '">' +
        '<input type="hidden" name="unit_price" value="' + unitPrice + '">';

      tbody.appendChild(tr);
    });

    // Show items section
    var itemsSection = document.getElementById("pr-items-section");
    if (itemsSection) itemsSection.style.display = "";

    // Bind oninput for subtotal recalculation
    tbody.querySelectorAll('input[name="returned_qty"]').forEach(function (input) {
      input.addEventListener("input", function () {
        PRCreate.recalcSubtotal(this);
      });
    });
  });

  // ── Helpers ──
  function esc(s) {
    if (!s) return "";
    var d = document.createElement("div");
    d.textContent = s;
    return d.innerHTML;
  }

  function fmtNum(n) {
    if (!n) return "0.00";
    var v = parseFloat(n);
    if (isNaN(v)) return "0.00";
    return v.toFixed(2);
  }

  // ── Global API ──
  window.PRCreate = {
    recalcSubtotal: function (qtyInput) {
      var tr = qtyInput.closest("tr");
      if (!tr) return;
      var priceInput = tr.querySelector('input[name="unit_price"]');
      var subtotalTd = tr.querySelector(".line-subtotal");
      if (!priceInput || !subtotalTd) return;

      var qty = parseFloat(qtyInput.value) || 0;
      var price = parseFloat(priceInput.value) || 0;
      subtotalTd.textContent = (qty * price).toFixed(2);
    },

    recalcLineNos: function () {
      var tbody = document.getElementById("pr-item-tbody");
      if (!tbody) return;
      tbody.querySelectorAll("tr").forEach(function (tr, i) {
        var td = tr.querySelector(".line-num");
        if (td) td.textContent = i + 1;
      });
    },

    collectItems: function () {
      var items = [];
      var tbody = document.getElementById("pr-item-tbody");
      if (!tbody) return;
      var intFields = ["order_item_id", "product_id"];
      tbody.querySelectorAll("tr").forEach(function (tr) {
        var vals = {};
        tr.querySelectorAll("input, select").forEach(function (el) {
          if (el.name) {
            var v = el.value;
            vals[el.name] = intFields.indexOf(el.name) >= 0 ? parseInt(v, 10) : v;
          }
        });
        items.push(vals);
      });
      document.getElementById("items-json").value = JSON.stringify(items);
    },
  };
})();
