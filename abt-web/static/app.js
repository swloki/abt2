// ── Toast ──

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
