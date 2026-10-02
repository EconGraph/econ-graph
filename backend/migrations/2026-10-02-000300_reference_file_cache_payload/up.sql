-- What an adapter kept from the last copy of a reference file it fetched, so a conditional GET
-- that comes back 304 can still be used without re-downloading the file. BLS keeps the title,
-- frequency and index base of each curated series from its survey `.series` files here. NULL for
-- files whose contents are merged elsewhere (e.g. dataset dimension codes).
ALTER TABLE reference_file_cache ADD COLUMN payload JSONB;
