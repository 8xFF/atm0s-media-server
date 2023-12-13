use std::fmt::Debug;

use crate::rpc::{RpcEndpoint, RpcReqRes, RpcRequest};
use atm0s_sdn::{RouteRule, RpcBox, RpcEmitter, RpcError, RpcMsg};

pub struct RpcRequestSdn {
    req: RpcMsg,
    emitter: RpcEmitter,
}

impl RpcRequest for RpcRequestSdn {
    fn cmd(&self) -> &str {
        &self.req.cmd
    }

    fn parse<P: for<'a> TryFrom<&'a [u8]> + Send + 'static, R: Into<Vec<u8>> + Send + 'static>(self) -> Option<Box<dyn RpcReqRes<P, R>>> {
        if let Some(req) = self.emitter.parse_request(self.req) {
            Some(Box::new(RpcReqResSdn { req }))
        } else {
            None
        }
    }

    fn error(self, err: &str) {
        let req = self.emitter.parse_request::<Vec<u8>, Vec<u8>>(self.req).expect("Vec<u8> must ok");
        req.error(err);
    }
}

pub struct RpcReqResSdn<P: for<'a> TryFrom<&'a [u8]> + Send + 'static, R: Into<Vec<u8>> + Send + 'static> {
    req: atm0s_sdn::RpcRequest<P, R>,
}

impl<P: for<'a> TryFrom<&'a [u8]> + Send + 'static, R: Into<Vec<u8>> + Send + 'static> RpcReqRes<P, R> for RpcReqResSdn<P, R> {
    fn param(&self) -> &P {
        self.req.param()
    }

    fn answer(&self, res: Result<R, &str>) {
        match res {
            Ok(res) => self.req.success(res),
            Err(e) => self.req.error(e),
        }
    }
}

impl<P: for<'a> TryFrom<&'a [u8]> + Send + 'static, R: Into<Vec<u8>> + Send + 'static> Debug for RpcReqResSdn<P, R> {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Result::Ok(())
    }
}

pub struct RpcEndpointSdn {
    pub(crate) rpc_box: RpcBox,
}

#[async_trait::async_trait]
impl RpcEndpoint<RpcRequestSdn, RcpEmitterSdn> for RpcEndpointSdn {
    fn emitter(&mut self) -> RcpEmitterSdn {
        RcpEmitterSdn { emitter: self.rpc_box.emitter() }
    }

    async fn recv(&mut self) -> Option<RpcRequestSdn> {
        loop {
            let rpc = self.rpc_box.recv().await?;
            if rpc.is_request() {
                return Some(RpcRequestSdn {
                    req: rpc,
                    emitter: self.rpc_box.emitter(),
                });
            }
        }
    }
}

#[derive(Clone)]
pub struct RcpEmitterSdn {
    pub(crate) emitter: RpcEmitter,
}

#[async_trait::async_trait]
impl crate::rpc::RpcEmitter for RcpEmitterSdn {
    fn emit<E: Into<Vec<u8>>>(&self, service: u8, node: Option<u32>, cmd: &str, event: E) {
        let rule = match node {
            Some(node) => RouteRule::ToNode(node),
            _ => RouteRule::ToService(0),
        };
        self.emitter.emit(service, rule, cmd, event);
    }

    async fn request<Req: Into<Vec<u8>> + Send, Res: for<'a> TryFrom<&'a [u8]> + Send>(
        &self,
        service: u8,
        node: Option<u32>,
        cmd: &str,
        req: Req,
        timeout_ms: u64,
    ) -> Result<Res, crate::rpc::RpcError> {
        let rule = match node {
            Some(node) => RouteRule::ToNode(node),
            _ => RouteRule::ToService(0),
        };
        self.emitter.request(service, rule, cmd, req, timeout_ms).await.map_err(|e| match e {
            RpcError::Timeout => crate::rpc::RpcError::Timeout,
            RpcError::DeserializeError => crate::rpc::RpcError::DeserializeError,
            RpcError::RemoteQueueError => crate::rpc::RpcError::RemoteQueueError,
            RpcError::LocalQueueError => crate::rpc::RpcError::LocalQueueError,
            RpcError::RuntimeError(e) => crate::rpc::RpcError::RuntimeError(e),
        })
    }
}
