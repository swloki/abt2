function reconciliationForm() {
    return {
        customerId: '',
        period: '',

        get canPreview() {
            return this.customerId && this.period;
        },

        triggerPreview() {
            if (!this.canPreview) return;
            var el = document.getElementById('rec-preview-area');
            if (el) {
                htmx.trigger(el, 'previewChanged');
            }
        }
    };
}
