use cluster::rpc::sip::{
    SipIncomingAuthRequest, SipIncomingAuthResponse, SipIncomingInviteRequest, SipIncomingInviteResponse, SipIncomingRegisterRequest, SipIncomingRegisterResponse, SipIncomingUnregisterRequest,
    SipIncomingUnregisterResponse,
};

#[derive(Debug, Clone)]
pub struct HooksSender {
    hook_url: String,
}

impl HooksSender {
    pub fn new(hook_url: &str) -> Self {
        Self { hook_url: hook_url.to_string() }
    }

    pub async fn hook_auth(&self, req: SipIncomingAuthRequest) -> Result<SipIncomingAuthResponse, String> {
        let client = reqwest::Client::new();
        client
            .post(format!("{}/auth", self.hook_url))
            .json(&req)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<SipIncomingAuthResponse>()
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn hook_register(&self, req: SipIncomingRegisterRequest) -> Result<SipIncomingRegisterResponse, String> {
        let client = reqwest::Client::new();
        client
            .post(format!("{}/register", self.hook_url))
            .json(&req)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<SipIncomingRegisterResponse>()
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn hook_unregister(&self, req: SipIncomingUnregisterRequest) -> Result<SipIncomingUnregisterResponse, String> {
        let client = reqwest::Client::new();
        client
            .post(format!("{}/unregister", self.hook_url))
            .json(&req)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<SipIncomingUnregisterResponse>()
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn hook_invite(&self, req: SipIncomingInviteRequest) -> Result<SipIncomingInviteResponse, String> {
        let client = reqwest::Client::new();
        client
            .post(format!("{}/invite", self.hook_url))
            .json(&req)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<SipIncomingInviteResponse>()
            .await
            .map_err(|e| e.to_string())
    }
}
