-- Version 2 semantic identity/content evidence. Runtime timestamps and mock HTTP origins are
-- excluded; names, unit/component metadata, exact nullable decimals, nullable observation
-- attributes (including captured-empty objects) and all read identities stay.
SELECT jsonb_build_object(
  'format', 2,
  'migrations', (SELECT coalesce(jsonb_agg(version ORDER BY version),'[]') FROM __diesel_schema_migrations),
  'sources', (SELECT coalesce(jsonb_agg(jsonb_build_object('id',id,'name',name,'is_visible',is_visible,'is_enabled',is_enabled) ORDER BY id),'[]') FROM data_sources),
  'datasets', (SELECT coalesce(jsonb_agg(jsonb_build_object('id',id,'source_id',source_id,'code',code,'name',name,'description',description,'dimensions',dimensions,'measures',measures,'attributes',attributes,'default_measure',default_measure) ORDER BY id),'[]') FROM datasets),
  'series', (SELECT coalesce(jsonb_agg(jsonb_build_object('id',id,'source_id',source_id,'external_id',external_id,'title',title,'description',description,'units',units,'frequency',frequency,'dataset_id',dataset_id,'dimensions',dimensions,'default_measure',default_measure,'is_active',is_active,'start_date',start_date,'end_date',end_date) ORDER BY id),'[]') FROM economic_series),
  'discovery', (SELECT coalesce(jsonb_agg(jsonb_build_object('id',id,'source_id',source_id,'external_id',external_id,'title',title,'description',description,'units',units,'frequency',frequency,'dataset_id',dataset_id,'dimensions',dimensions,'default_measure',default_measure,'is_active',is_active) ORDER BY id),'[]') FROM series_metadata),
  'observations', (SELECT coalesce(jsonb_agg(jsonb_build_object('id',id,'series_id',series_id,'date',date,'revision_date',revision_date,'is_original_release',is_original_release,'value',value::text,'attributes',attributes) ORDER BY id),'[]') FROM data_points)
)::text;
