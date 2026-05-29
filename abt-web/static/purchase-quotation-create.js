function purchaseQuotationForm() {
    return {
        items: [],
        productModalOpen: false,

        addItem(product) {
            this.items.push({
                product_id: product.product_id,
                product_code: product.product_code,
                product_name: product.product_name,
                unit_price: '0',
                min_order_qty: '',
                lead_time_days: '',
                currency: 'CNY',
                is_preferred: false
            });
            this.productModalOpen = false;
        },

        removeItem(idx) {
            this.items.splice(idx, 1);
        },

        get itemsJson() {
            return JSON.stringify(this.items.map(function (i) {
                return {
                    product_id: i.product_id,
                    unit_price: i.unit_price || '0',
                    min_order_qty: i.min_order_qty || null,
                    lead_time_days: i.lead_time_days || null,
                    currency: i.currency || 'CNY',
                    is_preferred: i.is_preferred ? 'on' : null
                };
            }));
        }
    };
}
