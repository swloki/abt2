var notyf = new Notyf({ duration: 5000, position: { x: 'right', y: 'top' }, dismissible: true });

function returnForm() {
    return {
        selectedOrderId: '',
        selectedCustomerId: '',
        selectedOrderDoc: '',
        shippingRequestId: '0',
        returnReason: '',
        items: [],

        selectOrder(order) {
            this.selectedOrderId = order.order_id;
            this.selectedCustomerId = order.customer_id;
            this.selectedOrderDoc = order.doc_number;
            // Load order items
            var self = this;
            fetch('/admin/returns/order-items?order_id=' + order.order_id)
                .then(function (r) { return r.text(); })
                .then(function (html) {
                    // Parse returned data to populate items
                    // The response contains item data as JSON in the template
                    try {
                        var data = JSON.parse(html);
                        self.items = data.items.map(function (item) {
                            return {
                                order_item_id: item[0],
                                product_id: item[2],
                                product_name: item[7] || '',
                                description: item[3],
                                order_qty: item[4],
                                unit: item[5],
                                unit_price: item[6],
                                returned_qty: String(item[4]),
                                disposition: '1'
                            };
                        });
                        if (data.shipping_id && data.shipping_id > 0) {
                            self.shippingRequestId = String(data.shipping_id);
                        }
                    } catch (e) {
                        // Fallback: try to extract from HTML
                        self.items = [];
                    }
                });
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
                    disposition: parseInt(i.disposition) || 1
                };
            }));
        }
    };
}

document.addEventListener('DOMContentLoaded', function () {
    var form = document.getElementById('return-form');
    if (form) {
        form.addEventListener('htmx:responseError', function (e) {
            notyf.error(e.detail.xhr.responseText || '提交失败');
        });
    }
});
