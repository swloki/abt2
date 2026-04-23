-- Migration 027: Rollback labor process routing tables
-- Drops bom_routing, routing_step, routing, labor_process_dict
-- Removes process_code column from bom_labor_process

BEGIN;

DROP TABLE IF EXISTS bom_routing;
DROP TABLE IF EXISTS routing_step;
DROP TABLE IF EXISTS routing;
DROP TABLE IF EXISTS labor_process_dict;

ALTER TABLE bom_labor_process DROP COLUMN IF EXISTS process_code;

DROP SEQUENCE IF EXISTS labor_process_dict_code_seq;

COMMIT;
