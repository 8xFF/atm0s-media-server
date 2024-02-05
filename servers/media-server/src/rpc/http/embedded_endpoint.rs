use std::marker::PhantomData;

use poem::{endpoint::EmbeddedFileEndpoint, Endpoint, Error, Request, Response};
use rust_embed::RustEmbed;

/// An endpoint that wraps a `rust-embed` bundle.
pub struct EmbeddedFilesEndpoint<E: RustEmbed + Send + Sync> {
    _embed: PhantomData<E>,
    index_file: Option<String>,
}

impl<E: RustEmbed + Sync + Send> Default for EmbeddedFilesEndpoint<E> {
    #[inline]
    fn default() -> Self {
        Self::new(None)
    }
}

impl<E: RustEmbed + Send + Sync> EmbeddedFilesEndpoint<E> {
    /// Create a new `EmbeddedFilesEndpoint` from a `rust-embed` bundle.
    pub fn new(index_file: Option<String>) -> Self {
        EmbeddedFilesEndpoint { _embed: PhantomData, index_file }
    }
}

#[async_trait::async_trait]
impl<E: RustEmbed + Send + Sync> Endpoint for EmbeddedFilesEndpoint<E> {
    type Output = Response;

    async fn call(&self, req: Request) -> Result<Self::Output, Error> {
        let mut path = req.uri().path().trim_start_matches('/').trim_end_matches('/').to_string();

        if path.is_empty() {
            path = "index.html".to_string();
        }

        if let Some(index_file) = &self.index_file {
            if E::get(&path).is_none() {
                path = format!("{path}/{index_file}")
            }
        }

        let path = path.as_ref();
        EmbeddedFileEndpoint::<E>::new(path).call(req).await
    }
}
