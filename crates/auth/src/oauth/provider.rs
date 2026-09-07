//! An OAuth identity provider.

#[derive(Debug, Clone)]
pub struct OAuthProvider {
    pub name: String,
    pub client_id: String,
    pub redirect_uri: String,
}

impl OAuthProvider {
    pub fn new(name: impl Into<String>, client_id: impl Into<String>, redirect_uri: impl Into<String>) -> Self {
        Self { name: name.into(), client_id: client_id.into(), redirect_uri: redirect_uri.into() }
    }

    pub fn authorize_url(&self, state: &str) -> String {
        format!("https://{}/authorize?client_id={}&state={}", self.name, self.client_id, state)
    }
}
