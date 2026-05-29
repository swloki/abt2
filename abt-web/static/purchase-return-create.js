function purchaseReturnForm() {
    return {
        selectedOrderId: '',
        returnReason: '',
        returnReasonDetail: '',
        items: [],

        loadOrderItems() {
            if (!this.selectedOrderId) {
                this.items = [];
                return;
            }
            var self = this;
            self.items = [];
            fetch('/admin/purchase/returns/order-items?order_id=' + encodeURIComponent(this.selectedOrderId), {
                headers: { 'HX-Request': 'true' }
            })
            .then(function (res) { return res.text(); })
            .then(function (html) {
                var parser = new DOMParser();
                var doc = parser.parseFromString(html, 'text/html');
                var inits = doc.querySelectorAll('[data-item]');
                inits.forEach(function (el) {
                    try {
                        var item = JSON.parse(el.getAttribute('data-item'));
                        self.addItem(item);
                    } catch (e) { /* skip */ }
                });
            });
        },

        addItem(item) {
            var exists = this.items.some(function (i) {
                return i.order_item_id === item.order_item_id;
            });
            if (!exists) {
                this.items.push(item);
            }
        },

        removeItem(idx) {
            this.items.splice(idx, 1);
        },

        get itemsJson() {
            return JSON.stringify(this.items.map(function (i) {
                return {
                    order_item_id: i.order_item_id,
                    product_id: i.product_id,
                    returned_qty: i.returned_qty || '1',
                    unit_price: i.unit_price || '0'
                };
            }));
        }
    };
}
