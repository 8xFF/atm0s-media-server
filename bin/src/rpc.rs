pub struct Rpc<Req, Res> {
    pub req: Req,
    pub answer_tx: tokio::sync::oneshot::Sender<Res>,
}

impl<Req, Res> Rpc<Req, Res> {
    pub fn new(req: Req) -> (Self, tokio::sync::oneshot::Receiver<Res>) {
        let (answer_tx, answer_rx) = tokio::sync::oneshot::channel();
        (Self { req, answer_tx }, answer_rx)
    }

    #[allow(unused)]
    pub fn res(self, res: Res) {
        let _ = self.answer_tx.send(res);
    }
}
