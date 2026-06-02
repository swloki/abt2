// ── Floating Dropdown Positioning ──
// Positions a dropdown relative to its trigger, auto-flipping when near edges.
// Usage: call positionDropdown(triggerEl, dropdownEl) when dropdown becomes visible.

window.positionDropdown = function (trigger, dropdown) {
    var rect = trigger.getBoundingClientRect();
    var menuRect = dropdown.getBoundingClientRect();
    var viewH = window.innerHeight;
    var viewW = window.innerWidth;

    // Default: below trigger, right-aligned
    var top = rect.bottom + 4;
    var left = rect.right - menuRect.width;

    // Flip above if not enough space below
    if (top + menuRect.height > viewH - 8 && rect.top - menuRect.height - 4 > 8) {
        top = rect.top - menuRect.height - 4;
    }
    // Clamp horizontal
    if (left + menuRect.width > viewW - 8) left = viewW - menuRect.width - 8;
    if (left < 8) left = 8;

    dropdown.style.position = 'fixed';
    dropdown.style.top = top + 'px';
    dropdown.style.left = left + 'px';
    dropdown.style.right = 'auto';
};


window.showToast = function (message, type) {
    var el = document.querySelector('.toast-container');
    if (el && el._x_dataStack) {
        // Alpine instance ready, push directly
        var data = el._x_dataStack[0];
        var t = { id: Date.now(), message: message, type: type || 'success' };
        data.toasts.push(t);
        setTimeout(function () {
            data.toasts = data.toasts.filter(function (x) { return x.id !== t.id; });
        }, 3000);
    } else {
        // Alpine not ready, dispatch event as fallback
        window.dispatchEvent(new CustomEvent('show-toast', { detail: { message: message, type: type || 'success' } }));
    }
};

// ── HTMX global error handling ──

document.addEventListener('htmx:afterRequest', function (e) {
    if (e.detail.successful) return;
    var xhr = e.detail.xhr;
    if (!xhr) return;

    if (xhr.status === 401) {
        window.location.href = '/login';
        return;
    }

    var msg = (xhr.responseText || '').trim() || '操作失败';
    window.showToast(msg, 'error');
});

// ── HTMX custom confirm dialog (replaces native confirm()) ──

document.addEventListener('htmx:confirm', function (e) {
    if (!e.detail.question) return;
    e.preventDefault();
    var el = document.getElementById('global-confirm-dialog');
    if (!el || !el._x_dataStack) {
        // fallback to native confirm if Alpine not ready
        if (confirm(e.detail.question)) e.detail.issueRequest(true);
        return;
    }
    var data = el._x_dataStack[0];
    data.confirmMessage = e.detail.question;
    data.confirmOpen = true;
    data._issueRequest = e.detail.issueRequest.bind(e.detail);
});

// ── Category Tree Select Component ──

window.categoryTreeSelect = function () {
    return {
        open: false,
        search: '',
        items: [],
        selectedId: null,
        selectedName: '',
        allLabel: '全部分类',

        init() {
            var raw = this.$el.dataset.ct;
            if (!raw) return;
            var data = JSON.parse(raw);
            this.items = data.items || [];
            this.selectedId = data.selected_id || null;
            this.allLabel = data.all_label || '全部分类';
            if (this.selectedId) {
                var self = this;
                var found = this.items.find(function (i) { return i.id === self.selectedId; });
                this.selectedName = found ? found.name : this.allLabel;
            } else {
                this.selectedName = this.allLabel;
            }
        },

        get filteredItems() {
            if (!this.search) return this.items;
            var q = this.search.toLowerCase();
            return this.items.filter(function (i) {
                return i.name.toLowerCase().indexOf(q) !== -1;
            });
        },

        toggle() {
            this.open = !this.open;
            if (!this.open) this.search = '';
        },

        close() {
            this.open = false;
            this.search = '';
        },

        select(value) {
            var id = typeof value === 'number' ? String(value) : value;
            var container = this.$el.closest('.tree-select');
            var input = container.querySelector('input[type=hidden]');
            input.value = id;
            if (id) {
                var found = this.items.find(function (i) { return String(i.id) === id; });
                this.selectedName = found ? found.name : this.allLabel;
            } else {
                this.selectedName = this.allLabel;
                this.selectedId = null;
            }
            input.dispatchEvent(new Event('change', { bubbles: true }));
            this.close();
        }
    };
};
