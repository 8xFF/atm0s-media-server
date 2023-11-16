use std::net::{IpAddr, SocketAddr};

use combine::attempt;
use combine::error::StreamError;
use combine::{
    choice, many, many1, optional,
    parser::char::{char, digit, string},
    satisfy,
    stream::StreamErrorFor,
    ParseError, Parser, Stream,
};
use str0m::{net::Protocol, Candidate, CandidateKind};

/// Not SP, \r or \n
fn not_sp<Input>() -> impl Parser<Input, Output = String>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    many1(satisfy(|c| c != ' ' && c != '\r' && c != '\n'))
}

// candidate:3390217324 1 udp 2122260223 10.243.178.47 53311 typ host generation 0 ufrag JtxE network-id
pub fn candidate<Input>() -> impl Parser<Input, Output = Candidate>
where
    Input: Stream<Token = char>,
    Input::Error: ParseError<Input::Token, Input::Range, Input::Position>,
{
    let port = || not_sp::<Input>().and_then(|s| s.parse::<u16>().map_err(StreamErrorFor::<Input>::message_format));

    let ip_addr = || not_sp().and_then(|s| s.parse::<IpAddr>().map_err(StreamErrorFor::<Input>::message_format));

    (
        string("candidate:"),
        many::<String, _, _>(digit()), //foundtion
        char(' '),
        not_sp().and_then(|s| s.parse::<u16>().map_err(StreamErrorFor::<Input>::message_format)), //component_id
        char(' '),                                                                                // protocol
        choice((
            string("udp").map(|_| Protocol::Udp),
            string("tcp").map(|_| Protocol::Tcp),
            string("ssltcp").map(|_| Protocol::SslTcp),
            string("relay").map(|_| Protocol::Tls),
        )),
        char(' '), // priority
        not_sp().and_then(|s| s.parse::<u32>().map_err(StreamErrorFor::<Input>::message_format)),
        char(' '), // ip
        ip_addr(),
        char(' '), // port
        port(),
        char(' '), // typ
        string("typ"),
        char(' '),
        choice((
            // typ
            string("host").map(|_| CandidateKind::Host),
            string("prflx").map(|_| CandidateKind::PeerReflexive),
            string("srflx").map(|_| CandidateKind::ServerReflexive),
            string("relay").map(|_| CandidateKind::Relayed),
        )),
        optional((attempt(string(" generation ")), not_sp())),
        optional((attempt(string(" raddr ")), ip_addr(), string(" rport "), port())),
        optional((attempt(string(" ufrag ")), not_sp())),
        optional((attempt(string(" network-id ")), not_sp())),
    )
        .map(
            |(_, foundation, _, component_id, _, proto, _, priority, _, ip, _, port, _, _, _, typ, _generation, raddr, ufrag, _network_id)| {
                Candidate::parsed(
                    foundation,
                    component_id,
                    proto,
                    priority,
                    SocketAddr::new(ip, port),
                    typ,
                    raddr.map(|(_, addr, _, port)| SocketAddr::from((addr, port))),
                    ufrag.map(|(_, u)| u),
                )
            },
        )
}

#[cfg(test)]
mod test {
    use std::net::Ipv4Addr;
    use super::*;

    #[test]
    fn test_candidate() {
        let mut candidate_parse = candidate();
        assert_eq!(
            candidate_parse.parse("candidate:3390217324 1 udp 2122260223 10.243.178.47 53311 typ host generation 0 ufrag JtxE network-id 2"),
            Ok((
                Candidate::parsed(
                    "3390217324".to_string(),
                    1,
                    Protocol::Udp,
                    2122260223,
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 243, 178, 47)), 53311),
                    CandidateKind::Host,
                    None,
                    Some("JtxE".to_string()),
                ),
                ""
            ))
        );
    }
}
