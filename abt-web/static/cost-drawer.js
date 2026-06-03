function costDrawer(itemsJson, laborJson, warningsJson, bomId) {
    return {
        items: itemsJson.map(function (item) {
            return Object.assign({}, item, { tempPrice: null });
        }),
        laborItems: laborJson,
        warnings: warningsJson,
        warnOpen: false,
        bomId: bomId,

        init() {
            this._loadTempPrices();
        },

        get hasAllMaterialPrices() {
            return this.items.every(function (item) { return item.unitPrice || item.tempPrice; });
        },

        get laborIssue() {
            return this.laborItems.length > 0 && this.laborItems.every(function (item) { return item.unitPrice === '0'; });
        },

        get materialTotal() {
            var sum = 0;
            for (var i = 0; i < this.items.length; i++) {
                var item = this.items[i];
                var price = item.unitPrice || item.tempPrice;
                if (price) sum += parseFloat(price) * parseFloat(item.quantity);
            }
            return sum;
        },

        get laborTotal() {
            var sum = 0;
            for (var i = 0; i < this.laborItems.length; i++) {
                sum += parseFloat(this.laborItems[i].unitPrice) * parseFloat(this.laborItems[i].quantity);
            }
            return sum;
        },

        get tempCount() {
            var count = 0;
            for (var i = 0; i < this.items.length; i++) {
                if (!this.items[i].unitPrice && this.items[i].tempPrice) count++;
            }
            return count;
        },

        get totalCardClass() {
            if (!this.hasAllMaterialPrices || this.laborIssue) return 'total-warn';
            return 'total-ok';
        },

        get totalLabel() {
            return '总成本';
        },

        get totalSub() {
            if (!this.hasAllMaterialPrices && this.laborIssue) return '材料缺失单价，人工成本为0';
            if (!this.hasAllMaterialPrices) return '存在缺失单价';
            if (this.laborIssue) return '人工成本为0';
            return '已完成计算';
        },

        get totalHint() {
            if (!this.hasAllMaterialPrices && this.laborIssue) return '请补全材料单价并设置人工成本';
            if (!this.hasAllMaterialPrices) return '请补全所有材料单价';
            return '请设置人工成本单价';
        },

        _storageKey() {
            return 'bom-cost-temp-prices:' + this.bomId;
        },

        _loadTempPrices() {
            try {
                var raw = localStorage.getItem(this._storageKey());
                if (!raw) return;
                var map = JSON.parse(raw);
                for (var i = 0; i < this.items.length; i++) {
                    var key = String(this.items[i].productId);
                    if (map[key]) {
                        this.items[i].tempPrice = map[key];
                    }
                }
            } catch (e) { }
        },

        _saveTempPrices() {
            var map = {};
            for (var i = 0; i < this.items.length; i++) {
                if (!this.items[i].unitPrice && this.items[i].tempPrice) {
                    map[String(this.items[i].productId)] = this.items[i].tempPrice;
                }
            }
            localStorage.setItem(this._storageKey(), JSON.stringify(map));
        },

        setTempPrice(item, event) {
            var val = (event.target.value || '').trim();
            if (!val) return;
            var num = parseFloat(val);
            if (isNaN(num) || num < 0) {
                event.target.value = '';
                return;
            }
            item.tempPrice = val;
            this._saveTempPrices();
            event.target.value = '';
        },

        clearTempPrices() {
            for (var i = 0; i < this.items.length; i++) {
                if (!this.items[i].unitPrice) {
                    this.items[i].tempPrice = null;
                }
            }
            localStorage.removeItem(this._storageKey());
        },

        fmtCurrency(val) {
            if (val == null) return '-';
            return '\u00a5' + Number(val).toFixed(6);
        },

        fmtAmount(price, qty) {
            return '\u00a5' + (parseFloat(price) * parseFloat(qty)).toFixed(6);
        }
    };
}
