function bomEdit() {
    return {
        layerFilter: 0,
        collapsedNodes: {},
        allCollapsed: false,
        addModalOpen: false,
        addParentId: 0,
        addProductId: 0,
        addProductCode: '',
        addProductName: '',
        addProductUnit: '',

        editModalOpen: false,
        editNodeId: 0,
        editNode: {
            quantity: '',
            loss_rate: '',
            unit: '',
            work_center: '',
            position: '',
            remark: ''
        },

        saveAsOpen: false,
        saveAsName: '',
        deleteOpen: false,

        selectAddProduct(product) {
            this.addProductId = product.product_id;
            this.addProductCode = product.product_code;
            this.addProductName = product.product_name;
            this.addProductUnit = product.unit || '';
            this.submitAddNode();
        },

        submitAddNode() {
            if (!this.addProductId) return;
            var bomId = window.location.pathname.split('/')[4];
            var fields = {
                product_id: this.addProductId,
                parent_id: this.addParentId,
                quantity: '1',
                unit: this.addProductUnit
            };
            htmx.ajax('POST', '/admin/md/boms/' + bomId + '/nodes', {
                values: fields,
                swap: 'none',
                headers: {'HX-Request': 'true'}
            }).then(() => {
                this.addModalOpen = false;
            });
        },

        openEdit(nodeId, quantity, lossRate, unit, workCenter, position, remark) {
            this.editNodeId = nodeId;
            this.editNode = {
                quantity: quantity,
                loss_rate: lossRate,
                unit: unit,
                work_center: workCenter,
                position: position,
                remark: remark
            };
            var bomId = window.location.pathname.split('/')[4];
            var form = document.getElementById('bom-edit-node-form');
            form.action = '/admin/md/boms/' + bomId + '/nodes/' + nodeId;
            form.setAttribute('hx-post', form.action);
            this.editModalOpen = true;
        },

        openDelete(nodeId) {
            var bomId = window.location.pathname.split('/')[4];
            var form = document.getElementById('bom-node-delete-form');
            form.action = '/admin/md/boms/' + bomId + '/nodes/' + nodeId;
            form.setAttribute('hx-delete', form.action);
            this.deleteOpen = true;
        },

        openAddChild(parentId) {
            this.addParentId = parentId;
            this.addProductId = 0;
            this.addModalOpen = true;
        },

        toggleCollapse(nodeId) {
            this.collapsedNodes[nodeId] = !this.collapsedNodes[nodeId];
            this._saveCollapsed();
        },

        toggleAllCollapse() {
            this.allCollapsed = !this.allCollapsed;
            var tbody = document.getElementById('bom-sortable-tbody');
            if (!tbody) return;
            var parentRows = tbody.querySelectorAll('tr[data-node-id]');
            var parentIds = [];
            parentRows.forEach(function(r) {
                var nid = Number(r.dataset.nodeId);
                var pid = Number(r.dataset.parentId);
                var child = tbody.querySelector('tr[data-parent-id="' + nid + '"]');
                if (child) parentIds.push(nid);
            });
            var self = this;
            parentIds.forEach(function(nid) {
                self.collapsedNodes[nid] = self.allCollapsed;
            });
            this._saveCollapsed();
        },

        isNodeVisible(level, ancestors) {
            var filter = parseInt(this.layerFilter);
            if (filter !== 0 && filter !== level) return false;
            if (!ancestors) return true;
            var ids = ancestors.split(',');
            for (var i = 0; i < ids.length; i++) {
                if (this.collapsedNodes[ids[i]]) return false;
            }
            return true;
        },
        initSortable() {
            this._restoreCollapsed();
            var bomId = window.location.pathname.split('/')[4];
            var tbody = document.getElementById('bom-sortable-tbody');
            if (!tbody) return;
            var table = tbody.closest('table');

            var dragNodeId = null;
            var dragParentId = null;
            var descendantIds = new Set();
            var cachedGaps = [];
            var currentGapIndex = -1;

            // Fixed-position overlay — does not touch table DOM at all
            var indicator = document.createElement('div');
            indicator.className = 'bom-drop-indicator';
            indicator.style.display = 'none';
            document.body.appendChild(indicator);

            function getDescendants(nodeId) {
                var ids = new Set([nodeId]);
                var changed = true;
                while (changed) {
                    changed = false;
                    tbody.querySelectorAll('tr[data-node-id]').forEach(function(r) {
                        var pid = Number(r.dataset.parentId);
                        var nid = Number(r.dataset.nodeId);
                        if (ids.has(pid) && !ids.has(nid)) {
                            ids.add(nid);
                            changed = true;
                        }
                    });
                }
                return ids;
            }

            // Cache gap Y coordinates (viewport) at dragstart
            function cacheGaps() {
                cachedGaps = [];
                var allRows = Array.from(tbody.querySelectorAll('tr[data-node-id]'));
                var siblings = allRows.filter(function(r) {
                    return Number(r.dataset.parentId) === dragParentId
                        && !descendantIds.has(Number(r.dataset.nodeId));
                });
                for (var i = 0; i < siblings.length; i++) {
                    var rect = siblings[i].getBoundingClientRect();
                    cachedGaps.push({ y: rect.top, row: siblings[i], pos: 'top' });
                    if (i + 1 < siblings.length) {
                        var nextRect = siblings[i + 1].getBoundingClientRect();
                        cachedGaps.push({ y: (rect.bottom + nextRect.top) / 2, row: siblings[i], pos: 'bottom' });
                    } else {
                        cachedGaps.push({ y: rect.bottom, row: siblings[i], pos: 'bottom' });
                    }
                }
            }

            function findNearestGap(clientY) {
                var best = -1;
                var bestDist = Infinity;
                for (var i = 0; i < cachedGaps.length; i++) {
                    var dist = Math.abs(clientY - cachedGaps[i].y);
                    if (dist < bestDist) {
                        bestDist = dist;
                        best = i;
                    }
                }
                return best;
            }

            function hideIndicator() {
                indicator.style.display = 'none';
                currentGapIndex = -1;
            }

            function showIndicatorAt(gapIndex) {
                if (gapIndex === currentGapIndex) return;
                currentGapIndex = gapIndex;
                var gap = cachedGaps[gapIndex];
                var tableRect = table.getBoundingClientRect();
                indicator.style.display = 'block';
                indicator.style.top = (gap.y - 24) + 'px';
                indicator.style.left = tableRect.left + 'px';
                indicator.style.width = tableRect.width + 'px';
            }

            tbody.addEventListener('dragstart', function(e) {
                var row = e.target.closest('tr[data-node-id]');
                if (!row) return;
                dragNodeId = Number(row.dataset.nodeId);
                dragParentId = Number(row.dataset.parentId);
                descendantIds = getDescendants(dragNodeId);
                tbody.querySelectorAll('tr[data-node-id]').forEach(function(r) {
                    if (descendantIds.has(Number(r.dataset.nodeId))) {
                        r.classList.add('bom-dragging');
                    }
                });
                e.dataTransfer.effectAllowed = 'move';
                e.dataTransfer.setData('text/plain', String(dragNodeId));
                cacheGaps();
            });

            tbody.addEventListener('dragend', function() {
                tbody.querySelectorAll('.bom-dragging').forEach(function(r) {
                    r.classList.remove('bom-dragging');
                });
                hideIndicator();
                dragNodeId = null;
                dragParentId = null;
                descendantIds = new Set();
                cachedGaps = [];
            });

            tbody.addEventListener('dragover', function(e) {
                e.preventDefault();
                e.dataTransfer.dropEffect = 'move';
                if (!dragNodeId || cachedGaps.length === 0) return;
                var gapIndex = findNearestGap(e.clientY);
                if (gapIndex >= 0) showIndicatorAt(gapIndex);
            });

            tbody.addEventListener('dragleave', function(e) {
                var related = e.relatedTarget;
                if (related && tbody.contains(related)) return;
                hideIndicator();
            });

            tbody.addEventListener('drop', function(e) {
                e.preventDefault();
                if (!dragNodeId || currentGapIndex < 0) { hideIndicator(); return; }
                var gap = cachedGaps[currentGapIndex];
                var targetRow = gap.row;
                var isTop = gap.pos === 'top';
                hideIndicator();

                var tid = Number(targetRow.dataset.nodeId);
                if (descendantIds.has(tid)) return;
                if (Number(targetRow.dataset.parentId) !== dragParentId) return;

                var allRows = Array.from(tbody.querySelectorAll('tr[data-node-id]'));
                var tIdx = allRows.indexOf(targetRow);
                var beforeSiblingId;

                if (isTop) {
                    beforeSiblingId = tid;
                } else {
                    beforeSiblingId = '';
                    for (var i = tIdx + 1; i < allRows.length; i++) {
                        if (Number(allRows[i].dataset.parentId) === dragParentId) {
                            beforeSiblingId = Number(allRows[i].dataset.nodeId);
                            break;
                        }
                    }
                }

                fetch('/admin/md/boms/' + bomId + '/nodes/' + dragNodeId + '/move', {
                    method: 'POST',
                    headers: {'HX-Request': 'true', 'Content-Type': 'application/x-www-form-urlencoded'},
                    body: 'new_parent_id=' + dragParentId + '&before_sibling_id=' + beforeSiblingId
                }).then(function(resp) {
                    if (resp.ok) {
                        var loc = resp.headers.get('HX-Redirect');
                        window.location.href = loc || window.location.pathname;
                    } else {
                        return resp.text().then(function(msg) {
                            alert(msg || '移动失败');
                        });
                    }
                }).catch(function() { alert('网络错误，请重试'); });
            });
        },
        _storageKey() {
            return 'bom-collapsed-' + window.location.pathname.split('/')[4];
        },
        _saveCollapsed() {
            try {
                var data = {};
                for (var k in this.collapsedNodes) {
                    if (this.collapsedNodes[k]) data[k] = true;
                }
                sessionStorage.setItem(this._storageKey(), JSON.stringify(data));
            } catch(e) {}
        },
        _restoreCollapsed() {
            try {
                var raw = sessionStorage.getItem(this._storageKey());
                if (raw) this.collapsedNodes = JSON.parse(raw);
            } catch(e) {}
        }
    };
}
