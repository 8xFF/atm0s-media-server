# Cluster

Cluster feature is implemented following [RFC-0003-media-global-cluster](https://github.com/8xFF/rfcs/pull/3).
More info can be found in user guide [here](../../user-guide/features/cluster.md).

## Implementation details

The cluster module is implemented in the `cluster` package. It is responsible for managing the cluster of media servers.

Each time new peer joined to media-server, cluster will create a new `ClusterEndpoint` to attach to the peer.

The `ClusterEndpoint` is responsible for managing pubsub channels, and also room information for the peer.
We use event based communication to interact with `ClusterEndpoint`:

```rust
#[async_trait::async_trait]
pub trait ClusterEndpoint: Send + Sync {
    fn on_event(&mut self, event: ClusterEndpointOutgoingEvent) -> Result<(), ClusterEndpointError>;
    async fn recv(&mut self) -> Result<ClusterEndpointIncomingEvent, ClusterEndpointError>;
}
```
