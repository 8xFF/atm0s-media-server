# Integration

## Integration Methods

We support integration with other systems through the following methods:

- **Token generation logic and client SDK**: Generate and validate tokens for authentication.
- **HTTP API**: Manage, kick, and make calls using HTTP endpoints.
- **Message queue**: Handle events using a message queue. Currently, only Nats is supported, but it can easily be expanded to support more types.
- **SIP integration with Hooks**: Handle SIP integration with Hooks for authentication, registration, unregistration, and invitation.

## Token generation logic and client SDK

The approach of token generation and validation in the media server is designed to be efficient and secure. The server does not store any token information in its database and does not rely on external services for token validation. Instead, each node in the cluster validates tokens based on its configuration.

Currently, the media server supports static cluster secret for token generation. However, there are plans to expand this functionality in the future, including options such as public-private key and hierarchical key (bip32).

Supported token generation mechanisms:

| Type | Token | Status |
| --- | --- | --- |
| Static Secret | [JWT](https://jwt.io/) | Done |
| Public Private Key | [JWT](https://jwt.io/) | TODO |
| Hierarchical Key | [JWT](https://jwt.io/) | TODO |

## HTTP APIs

We have some HTTP APIs for managing, sessions. The APIs are defined as below:

| Method | Endpoint | Body | Response | Description |
| --- | --- | --- | --- | --- |
| DELETE | /sessions/:session_id | body here | return session_id | Close a session |

## External event handling with message queue

For processing events, we use a message queue. Currently, only Nats is supported, but it can easily be expanded to support more types. Each time a room or peer event occurs, it will be sent to the message queue and processed by the connector node.
Supported events please see [Protobuf](/packages/protocol/src/media_endpoint_log.proto))

## SIP integration with Hooks

## SIP Integration with Hooks

For SIP integration, we use Hooks. Currently, only HTTP Hooks are supported. Each time a SIP event occurs, it will be sent to the Hook Endpoint. To make an outgoing call, an external service needs to send an invite request to the HTTP endpoint of SIP.

Hooks:

| Type | Endpoint | Body | Response | Status |
| --- | --- | --- | --- | --- |
| Auth | /hooks/auth | body here | return SIP HA1 hash or reject | Done |
| Register | /hooks/register | body here | Save to db  | Done |
| Unregister | /hooks/unregister | body here | Remove from db | Done |
| Invite | /hooks/invite | body here | return action: reject, or move to room | Done |

Actions:

| Type | Endpoint| Body | Respinse | Description |
| --- | --- | --- | --- | --- |
| Invite | /actions/invite | body here | response here | For creating outgoing call |


### Working flow

#### Register

```mermaid
sequenceDiagram
    participant sip-client
    participant atm0s-sip-server
    participant 3rd-hooks
    sip-client ->> atm0s-sip-server: Register
    atm0s-sip-server ->> 3rd-hooks: post /hooks/auth
    3rd-hooks ->> atm0s-sip-server: return md5(user:realm:password)
    atm0s-sip-server ->> sip-client: return OK or ERR
    atm0s-sip-server ->> 3rd-hooks: post /hooks/register includes session_id (if above is OK)
```

#### Incoming call

```mermaid
sequenceDiagram
    participant sip-client
    participant atm0s-sip-server
    participant 3rd-hooks
    sip-client->>atm0s-sip-server: incomming call
    atm0s-sip-server ->> 3rd-hooks: post /hooks/invite includes from, to
    3rd-hooks ->> atm0s-sip-server: return action: Reject, Accept, WaitOthers, includes room info
    atm0s-sip-server->atm0s-sip-server: create endpoint and join room if Accept or WaitOthers
    atm0s-sip-server ->> sip-client: return Busy if above Reject
    atm0s-sip-server ->> sip-client: return Accept if above Accept
    atm0s-sip-server ->> sip-client: return Ringing if above WaitOthers
```

#### Outgoing call

Call to client (Zoiper or Linphone app ...)

```mermaid
sequenceDiagram
    participant sip-client
    participant atm0s-sip-server
    participant 3rd-hooks
    3rd-hooks ->> atm0s-sip-server: post /sip/invite/client includes session_id, room_info
    atm0s-sip-server->>sip-client: outgoing call
    atm0s-sip-server->atm0s-sip-server: create endpoint and join room as connecting
    atm0s-sip-server ->> 3rd-hooks: return call_id
    sip-client->>atm0s-sip-server: accept
    atm0s-sip-server->atm0s-sip-server: switch endpoint to connected
```