-- Sales order state transition definitions
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('SalesOrderStatus', '', 'Draft', NULL, 1),
    ('SalesOrderStatus', 'Draft', 'Confirmed', NULL, 2),
    ('SalesOrderStatus', 'Confirmed', 'InProduction', NULL, 3),
    ('SalesOrderStatus', 'InProduction', 'Completed', NULL, 4),
    ('SalesOrderStatus', 'InProduction', 'Shipped', NULL, 5),
    ('SalesOrderStatus', 'Draft', 'Cancelled', NULL, 6),
    ('SalesOrderStatus', 'Confirmed', 'Cancelled', NULL, 7),
    ('SalesOrderStatus', 'Shipped', 'Completed', NULL, 8)
ON CONFLICT DO NOTHING;

-- Quotation state transition definitions
INSERT INTO state_transition_defs (entity_type, from_state, to_state, trigger_event, sort_order) VALUES
    ('QuotationStatus', '', 'Draft', NULL, 1),
    ('QuotationStatus', 'Draft', 'Sent', NULL, 2),
    ('QuotationStatus', 'Sent', 'Accepted', NULL, 3),
    ('QuotationStatus', 'Sent', 'Rejected', NULL, 4),
    ('QuotationStatus', 'Sent', 'Expired', NULL, 5),
    ('QuotationStatus', 'Draft', 'Expired', NULL, 6)
ON CONFLICT DO NOTHING;
