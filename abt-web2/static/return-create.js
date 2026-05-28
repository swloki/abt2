function returnForm() {
    return {
        customerId: '0',
        selectedOrderId: '',
        selectedOrderNumber: '',
        shippingRequestId: '0',
        returnReason: '',
        orderModalOpen: false,
        items: [],

        selectOrder(orderData) {
            this.selectedOrderId = orderData.id;
            this.selectedOrderNumber = orderData.doc_number;
            this.shippingRequestId = String(orderData.shipping_id || '0');
            this.items = orderData.items.map(function (item) {
                return {
                    order_item_id: item.order_item_id,
                    product_id: item.product_id,
                    product_code: item.product_code,
                    product_name: item.product_name,
                    unit: item.unit || '',
                    order_qty: item.order_qty,
                    unit_price: item.unit_price,
                    returned_qty: String(item.order_qty),
                    disposition: '1'
                };
            });
            this.orderModalOpen = false;
        },

        clearOrder() {
            this.selectedOrderId = '';
            this.selectedOrderNumber = '';
            this.shippingRequestId = '0';
            this.items = [];
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
    if (!form) return;

    form.addEventListener('htmx:responseError', function (e) {
        var msg = e.detail.xhr.responseText || '提交失败';
        htmx.trigger(document.body, 'show-toast', { message: msg, type: 'error' });
    });
});
