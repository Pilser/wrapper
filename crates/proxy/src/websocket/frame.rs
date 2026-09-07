//! WebSocket frame helpers over tungstenite messages.

/// Minimal decoded frame.
#[derive(Clone, Debug)]
pub struct WsFrame {
    pub text: Option<String>,
    pub binary: Option<Vec<u8>>,
}

impl WsFrame {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: Some(text.into()),
            binary: None,
        }
    }

    pub fn binary(bytes: Vec<u8>) -> Self {
        Self {
            text: None,
            binary: Some(bytes),
        }
    }

    pub fn into_message(self) -> tokio_tungstenite::tungstenite::Message {
        if let Some(text) = self.text {
            tokio_tungstenite::tungstenite::Message::Text(text)
        } else {
            tokio_tungstenite::tungstenite::Message::Binary(self.binary.unwrap_or_default())
        }
    }

    pub fn from_message(msg: &tokio_tungstenite::tungstenite::Message) -> Self {
        if let Some(text) = msg.to_text().ok() {
            Self::text(text.to_string())
        } else {
            Self::binary(msg.clone().into_data())
        }
    }
}
