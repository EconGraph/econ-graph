-- The Bank of Canada data_sources row describes what the crawler's Valet adapter collects, and
-- its rate_limit_per_minute matches the crawler's Bank of Canada policy (1 request/second).
--
-- No migration seeds this row: the crawler creates it from DataSource::boc() the first time it
-- needs it, so a database where it already exists still has the earlier template's description
-- and rate. This updates that row in place; a database without the row is unchanged and gets
-- the current template when the crawler creates it. Only these two columns change, so the
-- source's id, enabled and visibility flags, and its series are untouched.

UPDATE data_sources
SET description = 'Bank of Canada Valet: the policy rate, CORRA, the prime rate, exchange rates against the Canadian dollar, Government of Canada benchmark bond yields, core CPI measures (CPI-trim, CPI-median, CPI-common), and the M2 and M2++ monetary aggregates',
    rate_limit_per_minute = 60,
    updated_at = NOW()
WHERE name = 'Bank of Canada (BoC)';
