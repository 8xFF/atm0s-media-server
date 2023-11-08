pub const INVITE_REQ: &str = "INVITE sip:1003@192.168.66.113;transport=UDP SIP/2.0\r
Via: SIP/2.0/UDP 192.168.66.155:59530;branch=z9hG4bK-524287-1---4900d58f2225595c;rport\r
Max-Forwards: 70\r
Contact: <sip:1002@192.168.66.155:59530;transport=UDP>\r
To: <sip:1003@192.168.66.113>\r
From: <sip:1002@192.168.66.113;transport=UDP>;tag=b3b27614\r
Call-ID: bDioe0g_lGydVf71NpTBnA..\r
CSeq: 1 INVITE\r
Allow: INVITE, ACK, CANCEL, BYE, NOTIFY, REFER, MESSAGE, OPTIONS, INFO, SUBSCRIBE\r
Content-Type: application/sdp\r
Supported: replaces, norefersub, extended-refer, timer, sec-agree, outbound, path, X-cisco-serviceuri\r
User-Agent: Zoiper v2.10.19.5\r
Allow-Events: presence, kpml, talk, as-feature-event\r
Content-Length: 264\r
\r
v=0\r
o=Z 0 199267607 IN IP4 192.168.66.155\r
s=Z\r
c=IN IP4 192.168.66.155\r
t=0 0\r
m=audio 61265 RTP/AVP 3 101 110 97 8 0\r
a=rtpmap:101 telephone-event/8000\r
a=fmtp:101 0-16\r
a=rtpmap:110 speex/8000\r
a=rtpmap:97 iLBC/8000\r
a=fmtp:97 mode=20\r
a=sendrecv\r
a=rtcp-mux\r
";

pub const ACK_REQ: &str = "ACK sip:192.168.66.113 SIP/2.0\r
Via: SIP/2.0/UDP 192.168.66.155:59530;branch=z9hG4bK-524287-1---3c9aaece04169f91;rport\r
Max-Forwards: 70\r
Contact: <sip:1002@192.168.66.155:59530;transport=UDP>\r
To: <sip:1003@192.168.66.113>\r
From: <sip:1002@192.168.66.113;transport=UDP>;tag=b3b27614\r
Call-ID: bDioe0g_lGydVf71NpTBnA..\r
CSeq: 1 ACK\r
User-Agent: Zoiper v2.10.19.5\r
Content-Length: 0\r\n\r\n";

pub const CANCEL_REQ: &str = "CANCEL sip:192.168.66.113 SIP/2.0\r
Via: SIP/2.0/UDP 192.168.66.155:59530;branch=z9hG4bK-524287-1---3c9aaece04169f91;rport\r
Max-Forwards: 70\r
Contact: <sip:1002@192.168.66.155:59530;transport=UDP>\r
To: <sip:1003@192.168.66.113>\r
From: <sip:1002@192.168.66.113;transport=UDP>;tag=b3b27614\r
Call-ID: bDioe0g_lGydVf71NpTBnA..\r
CSeq: 1 CANCEL\r
User-Agent: Zoiper v2.10.19.5\r
Content-Length: 0\r\n\r\n";

pub const BYE_REQ: &str = "BYE sip:192.168.66.113 SIP/2.0\r
Via: SIP/2.0/UDP 192.168.66.155:59530;branch=z9hG4bK-524287-1---b77e9fcb60843fbe;rport\r
Max-Forwards: 70\r
Contact: <sip:1002@192.168.66.155:59530;transport=UDP>\r
To: <sip:1003@192.168.66.113>\r
From: <sip:1002@192.168.66.113;transport=UDP>;tag=9c286553\r
Call-ID: prZODolN5IC_3JXBoSy3PA..\r
CSeq: 2 BYE\r
User-Agent: Zoiper v2.10.19.5\r
Content-Length: 0\r\n\r\n";

pub const BYE_RES: &str = "SIP/2.0 200 OK\r
Via: SIP/2.0/UDP 192.168.66.155:59530;branch=z9hG4bK-524287-1---b77e9fcb60843fbe;rport\r
From: <sip:1002@192.168.66.113;transport=UDP>;tag=9c286553\r
To: <sip:1003@192.168.66.113>\r
Call-ID: prZODolN5IC_3JXBoSy3PA..\r
CSeq: 2 BYE\r
Content-Length: 0\r
User-Agent: rsip\r\n\r\n";
