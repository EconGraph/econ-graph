-- Version 1 boundary fixture. UUIDs deliberately do not derive from source/external IDs.
INSERT INTO data_sources (id, name, base_url, created_at, updated_at)
VALUES ('30000000-0000-4000-8000-000000000001', 'Train 3 recorded fixture', 'https://invalid.example', '2026-01-01Z', '2026-01-01Z');
INSERT INTO datasets (id, source_id, code, name, measures, created_at, updated_at)
VALUES ('30000000-0000-4000-8000-000000000002', '30000000-0000-4000-8000-000000000001', 'BOUNDARY', 'Exact nullable revisions', '[{"name":"value","label":"Value","type":"decimal","unit":"USD"}]', '2026-01-01Z', '2026-01-01Z');
INSERT INTO economic_series (id, source_id, dataset_id, external_id, title, units, frequency, created_at, updated_at)
VALUES ('30000000-0000-4000-8000-000000000003', '30000000-0000-4000-8000-000000000001', '30000000-0000-4000-8000-000000000002', 'legacy-reference', 'Train 3 exact values', 'USD', 'Monthly', '2026-01-01Z', '2026-01-01Z');
INSERT INTO data_points (id, series_id, date, value, revision_date, is_original_release, created_at, updated_at) VALUES
('30000000-0000-4000-8000-000000000004', '30000000-0000-4000-8000-000000000003', '2026-01-01', 123456789012345678901234567890.123456789012345678901234567890, '2026-01-02', true, '2026-01-01Z', '2026-01-01Z'),
('30000000-0000-4000-8000-000000000005', '30000000-0000-4000-8000-000000000003', '2026-01-01', 123456789012345678901234567890.987654321098765432109876543210, '2026-02-02', false, '2026-01-01Z', '2026-01-01Z'),
('30000000-0000-4000-8000-000000000006', '30000000-0000-4000-8000-000000000003', '2026-02-01', NULL, '2026-02-02', true, '2026-01-01Z', '2026-01-01Z');
