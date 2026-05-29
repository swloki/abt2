function orderForm(initialItems) {
    return {
        items: initialItems || [],
        productModalOpen: false,

        addItem(product) {
            this.items.push({
                product_id: product.product_id,
                product_code: product.product_code,
                product_name: product.product_name,
                unit: product.unit || '',
                description: product.specification || '',
                quantity: '1',
                unit_price: '0',
                discount_rate: '0',
                delivery_date: ''
            });
            this.productModalOpen = false;
        },

        removeItem(idx) {
            this.items.splice(idx, 1);
        },

        subtotal(idx) {
            var item = this.items[idx];
            if (!item) return 0;
            return item.quantity * item.unit_price * (1 - item.discount_rate / 100);
        },

        get lineTotal() {
            return this.items.reduce(function (s, i) { return s + i.quantity * i.unit_price; }, 0);
        },

        get discountTotal() {
            return this.items.reduce(function (s, i) { return s + i.quantity * i.unit_price * (i.discount_rate / 100); }, 0);
        },

        get grandTotal() {
            return this.lineTotal - this.discountTotal;
        },

        get itemsJson() {
            return JSON.stringify(this.items.map(function (i) {
                return {
                    product_id: i.product_id,
                    description: i.description,
                    quantity: i.quantity || '1',
                    unit: i.unit,
                    unit_price: i.unit_price || '0',
                    discount_rate: i.discount_rate || '0',
                    delivery_date: i.delivery_date || null
                };
            }));
        }
    };
}
