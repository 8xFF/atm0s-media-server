# RTP Engine

The atm0s-media-server supports RTP transport for integration with external
applications such as SIP Gateways. We have also developed a simple SIP Gateway
that supports basic call flows, including outgoing and incoming calls:
[atm0s-sip-gateway](https://github.com/8xFF/atm0s-media-sip-gateway).

## Integration Guide

Our RTP media endpoint and HTTP APIs enable integration with external SIP
Gateways. The integration process involves three main steps:

1. Creating an RTP token
2. Creating an SDP Offer for outgoing calls or an SDP Answer for incoming calls
3. Deleting the RTP media endpoint after the call ends

For more detailed API documentation, please visit:

`http://gateway/token/ui` and `http://gateway/rtpengine/ui`

### RTPEngine APIs

The RTPEngine APIs provide endpoints to manage RTP media endpoints for SIP
integration. All endpoints are prefixed with `/rtpengine/`.

#### Authentication

Most endpoints require Bearer token authentication. Include your token in the
Authorization header:

```
Authorization: Bearer <your-token>
```

#### Endpoints

##### Create Offer

- **POST** `/rtpengine/offer`
- **Summary**: Creates an RTPEngine endpoint with an offer
- **Security**: Bearer token required
- **Response**:
  - Status: 200
  - Content-Type: application/sdp
  - Schema: SDP string

##### Create Answer

- **POST** `/rtpengine/answer`
- **Summary**: Creates an RTPEngine endpoint with an answer
- **Security**: Bearer token required
- **Request Body**:
  - Content-Type: application/sdp
  - Required: true
  - Schema: SDP string
- **Response**:
  - Status: 200
  - Content-Type: application/sdp
  - Schema: SDP string

##### Delete Connection

- **DELETE** `/rtpengine/conn/{conn_id}`
- **Summary**: Deletes an RTPEngine connection
- **Response**:
  - Status: 200
  - Content-Type: text/plain; charset=utf-8
  - Schema: string
