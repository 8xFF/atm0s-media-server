# Authentication and Multi Tenancy

We can extend with custom authentication by implement `SessionTokenSigner` trait.

```rust
pub trait SessionTokenSigner {
    fn sign_media_session(&self, token: &MediaSessionToken) -> String;
    fn sign_conn_id(&self, conn_id: &MediaConnId) -> String;
}

pub trait SessionTokenVerifier {
    fn verify_media_session(&self, token: &str) -> Option<MediaSessionToken>;
    fn verify_conn_id(&self, token: &str) -> Option<MediaConnId>;
}
```