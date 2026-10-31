use anyhow::{bail, Context, Result};
use iceberg::{
    io::{FileIO, FileIOBuilder, StorageFactory},
    memory::{MemoryCatalog, MemoryCatalogBuilder},
    CatalogBuilder,
};
use iceberg_storage_opendal::OpenDalStorageFactory;
use std::{collections::HashMap, sync::Arc};

/// Configuration values may include credentials. Deliberately does not implement Debug.
#[derive(Clone)]
pub struct StorageConfig {
    pub warehouse: String,
    pub properties: HashMap<String, String>,
}
impl StorageConfig {
    pub fn local(warehouse: impl Into<String>) -> Self {
        Self {
            warehouse: warehouse.into(),
            properties: HashMap::new(),
        }
    }
    pub fn from_env() -> Result<Self> {
        let warehouse =
            std::env::var("ICEBERG_WAREHOUSE").context("ICEBERG_WAREHOUSE is required")?;
        let mut properties = HashMap::new();
        for (env, key) in [
            ("S3_ENDPOINT", iceberg::io::S3_ENDPOINT),
            ("AWS_REGION", iceberg::io::S3_REGION),
            ("AWS_ACCESS_KEY_ID", iceberg::io::S3_ACCESS_KEY_ID),
            ("AWS_SECRET_ACCESS_KEY", iceberg::io::S3_SECRET_ACCESS_KEY),
            ("AWS_SESSION_TOKEN", iceberg::io::S3_SESSION_TOKEN),
        ] {
            if let Ok(value) = std::env::var(env) {
                properties.insert(key.into(), value);
            }
        }
        if let Some(endpoint) = properties.get(iceberg::io::S3_ENDPOINT) {
            let endpoint = url::Url::parse(endpoint).context("invalid S3 endpoint URL")?;
            if !matches!(endpoint.scheme(), "http" | "https")
                || endpoint.host_str().is_none()
                || !endpoint.username().is_empty()
                || endpoint.password().is_some()
                || endpoint.query().is_some()
                || endpoint.fragment().is_some()
            {
                bail!("S3 endpoint must be HTTP(S) with a host and no userinfo, query or fragment");
            }
        }
        if warehouse.starts_with("s3://") {
            properties.insert(iceberg::io::S3_PATH_STYLE_ACCESS.into(), "true".into());
            properties.insert(iceberg::io::S3_DISABLE_EC2_METADATA.into(), "true".into());
            if properties
                .get(iceberg::io::S3_ENDPOINT)
                .is_some_and(|e| e.starts_with("http://"))
                && std::env::var("AWS_ALLOW_HTTP").as_deref() != Ok("true")
            {
                bail!("HTTP object-store endpoint requires explicit AWS_ALLOW_HTTP=true");
            }
        }
        Ok(Self {
            warehouse,
            properties,
        })
    }
    fn factory(&self) -> Result<Arc<dyn StorageFactory>> {
        if self.warehouse.contains("://") {
            let warehouse = url::Url::parse(&self.warehouse).context("invalid warehouse URL")?;
            if !warehouse.username().is_empty()
                || warehouse.password().is_some()
                || warehouse.query().is_some()
                || warehouse.fragment().is_some()
            {
                bail!("warehouse URL must not contain userinfo, query or fragment");
            }
        }
        if self.warehouse.starts_with("s3://") {
            Ok(Arc::new(OpenDalStorageFactory::S3 {
                customized_credential_load: None,
            }))
        } else if self.warehouse.starts_with('/') || self.warehouse.starts_with("file://") {
            Ok(Arc::new(OpenDalStorageFactory::Fs))
        } else {
            bail!("warehouse must be an absolute local path, file:// URL or s3:// URL")
        }
    }
    pub fn file_io(&self) -> Result<FileIO> {
        Ok(FileIOBuilder::new(self.factory()?)
            .with_props(self.properties.clone())
            .build())
    }
    /// The upstream in-memory catalog is only a metadata staging builder. PostgreSQL below
    /// remains authoritative; this temporary object's pointer is never exposed to readers.
    pub(crate) async fn staging_catalog(&self) -> Result<MemoryCatalog> {
        let mut props = self.properties.clone();
        props.insert("warehouse".into(), self.warehouse.clone());
        Ok(MemoryCatalogBuilder::default()
            .with_storage_factory(self.factory()?)
            .load("staging", props)
            .await?)
    }
}
