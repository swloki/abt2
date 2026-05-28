function shippingForm(warehouses) {
    return {
        warehouses: warehouses || [],
        customerId: '',
        selectedOrderId: '',
        selectedOrderNumber: '',
        orderModalOpen: false,
        items: [],

        selectOrder(orderData) {
            this.selectedOrderId = orderData.id;
            this.selectedOrderNumber = orderData.doc_number;
            this.items = orderData.items.map(function (item) {
                return {
                    order_item_id: item.order_item_id,
                    product_id: item.product_id,
                    product_code: item.product_code,
                    product_name: item.product_name,
                    specification: item.specification || '',
                    unit: item.unit || '',
                    ordered_qty: item.ordered_qty,
                    shipped_qty: item.shipped_qty,
                    ship_qty: (parseFloat(item.ordered_qty) - parseFloat(item.shipped_qty)).toString(),
                    warehouse_id: ''
                };
            });
            this.orderModalOpen = false;
        },

        clearOrder() {
            this.selectedOrderId = '';
            this.selectedOrderNumber = '';
            this.items = [];
        },

        removeItem(idx) {
            this.items.splice(idx, 1);
        },

        get totalItems() {
            return this.items.length;
        },

        get totalQty() {
            return this.items.reduce(function (s, i) {
                return s + (parseFloat(i.ship_qty) || 0);
            }, 0);
        },

        get itemsJson() {
            return JSON.stringify(this.items.map(function (i) {
                return {
                    order_item_id: i.order_item_id,
                    warehouse_id: i.warehouse_id || 0,
                    requested_qty: i.ship_qty || '0'
                };
            }));
        }
    };
}

document.addEventListener('DOMContentLoaded', function () {
    var form = document.getElementById('shipping-form');
    if (!form) return;

    form.addEventListener('htmx:responseError', function (e) {
        var msg = e.detail.xhr.responseText || '提交失败';
        htmx.trigger(document.body, 'show-toast', { message: msg, type: 'error' });
    });
});
