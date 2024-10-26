# Integration

## Integration Methods

We support integration with other systems through the following methods:

- **Token generation logic and client SDK**: Generate and validate tokens for authentication.
- **HTTP API**: Create tokens and manage sessions using HTTP endpoints.
- **Hooks**: Trigger events to external logic via HTTP Hooks.
- **SIP integration with SIP-Gateway**: The atm0s-media-server supports RTP transport, allowing integration with external SIP-Gateways. We provide a simple SIP-Gateway for basic call flows, including outgoing and incoming calls.

## Token Generation and Validation

| Type          | Token                  | Status |
| ------------- | ---------------------- | ------ |
| Static Secret | [JWT](https://jwt.io/) | Done   |

The media server employs an efficient and secure approach for token generation and validation. It does not store any token information in its database and does not rely on external services for token validation. Instead, each node in the cluster validates tokens based on its configuration.

Currently, the media server uses JWT with a static cluster secret for token generation. It also supports multi-tenancy applications by synchronizing data from an external HTTP source that responds with a JSON structure like:

```json
{
  "apps": {
    "app1": {
      "secret": "secret1"
    },
    "app2": {
      "secret": "secret2"
    }
  }
}
```

The synchronization endpoint can be used with the `--multi-tenancy-sync` option of the gateway node. Instead of using the root secret, we can use the app secret to create tokens specific to that app.

We can use token generation APIs to create tokens. For more information, please refer to the HTTP APIs section below.

## HTTP APIs

We provide several HTTP APIs for managing sessions. The APIs are defined in an OpenAPI documentation page:

| Feature                 | Docs                     |
| ----------------------- | ------------------------ |
| Console Panel/User      | CONSOLE/api/user/ui      |
| Console Panel/Cluster   | CONSOLE/api/cluster/ui   |
| Console Panel/Connector | CONSOLE/api/connector/ui |
| Gateway Token           | GATEWAY/token/ui         |
| Gateway WebRTC          | GATEWAY/webrtc/ui        |
| Gateway WHIP            | GATEWAY/whip/ui          |
| Gateway WHEP            | GATEWAY/whep/ui          |
| Gateway RTP Engine      | GATEWAY/rtpengine/ui     |

## External Event Handling with Message Queue

For processing events, we utilize the HTTP Hooks mechanism.

## SIP Integration with RTP Engine Protocol

[SIP Integration](../getting-started/quick-start/sip.md)
