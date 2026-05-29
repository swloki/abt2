function purchaseOrderForm() {
    return {
        items: [],
        productModalOpen: false,

        addItem(product) {
            this.items.push({
                product_id: product.product_id,
                product_code: product.product_code,
                product_name: product.product_name,
                description: '',
                quantity: '1',
                unit_price: '0',
                expected_delivery_date: ''
            });
            this.productModalOpen = false;
        },

        removeItem(idx) {
            this.items.splice(idx, 1);
        },

        subtotal(idx) {
            var item = this.items[idx];
            if (!item) return 0;
            return item.quantity * item.unit_price;
        },

        get lineTotal() {
            return this.items.reduce(function (s, i) { return s + i.quantity * i.unit_price; }, 0);
        },

        get itemsJson() {
            return JSON.stringify(this.items.map(function (i) {
                return {
                    product_id: i.product_id,
                    description: i.description || null,
                    quantity: i.quantity || '1',
                    unit_price: i.unit_price || '0',
                    expected_delivery_date: i.expected_delivery_date || null
                };
            }));
        }
    };
}
