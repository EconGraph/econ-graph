# BLS series id columns (test fixtures)

Every row of BLS's `jt.series`, `wp.series` and `ei.series`
(`https://download.bls.gov/pub/time.series/{jt,wp,ei}/`), as downloaded on 2026-10-30 and
cut down to the series id and the code columns its id encodes. The BLS adapter's
`layouts_reproduce_the_series_files_codes` test checks that each id splits into the codes
BLS gives it. They are never used as reference data by the crawler.
