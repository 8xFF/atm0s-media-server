# SIP protocol information for implementing

- UAC (user agent client): which generates requests
- UAS (user agent server): which responds to them
- Dialogs: they represent a peer-to-peer relationship between user agents and are established by specific SIP methods, such as INVITE.

## SIP method-independent rules

### UAC Behavior

Regenerate request need minimum header fields: `[To, From, CSeq, Call-ID, Max-Forwards, Via]`

Process Response: x00 = xYY if xYY is unknown

Processing 3xx Responses => In order to create a request based on a contact address in a 3xx
   response, a UAC MUST copy the entire URI from the target set into the
   Request-URI, except for the "method-param" and "header" URI
   parameters

If a 401 (Unauthorized) or 407 (Proxy Authentication Required)
   response is received, the UAC SHOULD follow the authorization
   procedures of Section 22.2 and Section 22.3 to retry the request with
   credentials.

If create new req => should same Call-ID, To, From. CSeq add more 1

### UAS Behavior

Two request types: inside or outside of a dialog. (https://www.rfc-editor.org/rfc/rfc3261.html#section-12)

Request are atomic: accept or reject
Process request in order: authentication -> method -> header -> others

For authentication, read rfc for more info.

If not support -> Must generate 405 response with allow types in Alow header (how to generate response in https://www.rfceditor.org/rfc/rfc3261.html#section-8.2.6)

If support -> continue

Next is Header Inspection, ignore all header if unknown.

Next header: To and Request-URI, can ignore uri scheme in To if unknown; if reject send 403. If schema is not support reject with 416. If Request-URI not found in server reject with 404. 

Merged Requests: if no tag in To header => check in ongoging transactions for finding matching (From, Call-ID, CSeq) (matcching rule https://www.rfc-editor.org/rfc/rfc3261.html#section-17.2.3), if not => response 482 (Loop Detected)

After that processing as
- REGISTER: https://www.rfc-editor.org/rfc/rfc3261.html#section-11
- OPTIONS: https://www.rfc-editor.org/rfc/rfc3261.html#section-11
- INVITE: https://www.rfc-editor.org/rfc/rfc3261.html#section-13
- BYE: https://www.rfc-editor.org/rfc/rfc3261.html#section-15

How to generate general response:

- Provisional Response: not for non-INVITE request
    - Copy Timestamp header to 100 (Trying) response
- Headers and Tags:
    - Same From, Call-ID, CSeq, Via
    - If request has To tag => must keep, if not keep uri of request To and generate tag to response. (https://www.rfc-editor.org/rfc/rfc3261.html#section-19.3)

### Cancel request
UAS receive canceling request: best for INVITE, if not send final response => stop ringing and response to INVITE with code 487 more info (https://www.rfc-editor.org/rfc/rfc3261.html#section-15).


### Dialog

Identify with dialog ID, (consists of a Call-ID value, a local tag and a remote tag)
Generate rule
- UAC: the Call-ID value of the dialog ID is set to the
   Call-ID of the message, the remote tag is set to the tag in the To
   field of the message, and the local tag is set to the tag in the From
- UAS: the Call-ID value of the
   dialog ID is set to the Call-ID of the message, the remote tag is set
   to the tag in the From field of the message, and the local tag is set
   to the tag in the To field of the message.

Dialog state: a local sequence number, a remote sequence number, a local URI, a remote URI, remote target, a boolean
   flag called "secure", and a route set, which is an ordered list of
   URIs (The route set is the list of servers that need to be traversed
   to send a request to the peer)

##### For register

https://www.rfc-editor.org/rfc/rfc3261.html#section-10.2.8

##### For Options

Accept will be application/sdp

Response 200 if accepted, 486 if busy ...
