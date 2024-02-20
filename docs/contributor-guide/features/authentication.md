# Authentication

We can extend with custom authentication by implementing the `SessionTokenSigner` trait.

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

We have a simple static secret signer and verifier in the [`cluster` crate](https://github.com/8xFF/atm0s-media-server/blob/master/packages/cluster/src/implement/secure/jwt_static.rs).
