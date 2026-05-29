function miscRequestForm() {
    return {
        items: [],

        addItem() {
            this.items.push({
                item_name: '',
                specification: '',
                quantity: '1',
                unit: '',
                estimated_price: '',
                remark: ''
            });
        },

        removeItem(idx) {
            this.items.splice(idx, 1);
        },

        get lineTotal() {
            return this.items.reduce(function (s, i) {
                return s + (parseFloat(i.quantity) || 0) * (parseFloat(i.estimated_price) || 0);
            }, 0);
        },

        get itemsJson() {
            return JSON.stringify(this.items.map(function (i) {
                return {
                    item_name: i.item_name,
                    specification: i.specification || undefined,
                    quantity: i.quantity || '1',
                    unit: i.unit,
                    estimated_price: i.estimated_price || undefined,
                    remark: i.remark || undefined
                };
            }));
        }
    };
}
