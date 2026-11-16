//! Invocation-owned WebSocket connections over the managed outbound network policy.
use crate::{
    http::HttpBroker,
    policy::{error, Result},
};
use bgx_adapter_v2_ports::resource_policy;
use bgx_adapter_v2_transport::Control;
use futures_util::{SinkExt, StreamExt};
use reqwest::{header, StatusCode, Upgraded};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::time::{timeout_at, Instant};
use tokio_tungstenite::{
    tungstenite::{
        handshake::{client::generate_key, derive_accept_key},
        protocol::{Role, WebSocketConfig},
        Error, Message,
    },
    WebSocketStream,
};
use uuid::Uuid;

pub(crate) const FEATURE: &str = "websocket-v1";

struct Socket {
    stream: WebSocketStream<Upgraded>,
    max_bytes: usize,
}

pub(crate) struct WebSockets {
    http: Arc<HttpBroker>,
    sockets: HashMap<String, Socket>,
    enabled: bool,
}

fn limit(name: &str) -> usize {
    resource_policy()["limits"][name]
        .as_u64()
        .expect("public WebSocket limits") as usize
}

fn transport_error(err: Error) -> bgx_adapter_v2_ports::wire::host::HostError {
    if matches!(err, Error::Capacity(_)) {
        return error(
            "BUDGET_EXCEEDED",
            "WebSocket message exceeded its negotiated byte limit",
        );
    }
    error(
        "RESOURCE_UNAVAILABLE",
        "WebSocket connection was interrupted",
    )
}

fn websocket_http_url(raw: &str) -> Result<url::Url> {
    let mut url =
        url::Url::parse(raw).map_err(|_| error("PERMISSION_DENIED", "WebSocket URL is invalid"))?;
    let scheme = match url.scheme() {
        "ws" => "http",
        "wss" => "https",
        _ => {
            return Err(error(
                "PERMISSION_DENIED",
                "WebSocket URL must use ws or wss",
            ))
        }
    };
    url.set_scheme(scheme)
        .map_err(|_| error("PERMISSION_DENIED", "WebSocket URL scheme is invalid"))?;
    crate::policy::parse_url(url.as_str())
}

fn verify_upgrade(response: &reqwest::Response, key: &str) -> Result<()> {
    let headers = response.headers();
    let has_token = |name, target: &str| {
        headers.get_all(name).iter().any(|value| {
            value.to_str().is_ok_and(|text| {
                text.split(',')
                    .any(|token| token.trim().eq_ignore_ascii_case(target))
            })
        })
    };
    let expected = derive_accept_key(key.as_bytes());
    let accept = headers.get_all("sec-websocket-accept");
    let mut values = accept.iter();
    if response.status() != StatusCode::SWITCHING_PROTOCOLS
        || !has_token(header::CONNECTION, "upgrade")
        || !has_token(header::UPGRADE, "websocket")
        || values.next().and_then(|value| value.to_str().ok()) != Some(&expected)
        || values.next().is_some()
        || headers.contains_key("sec-websocket-extensions")
        || headers.contains_key("sec-websocket-protocol")
    {
        return Err(error(
            "RESOURCE_UNAVAILABLE",
            "Remote endpoint refused the WebSocket handshake",
        ));
    }
    Ok(())
}

impl WebSockets {
    pub(crate) fn new(http: Arc<HttpBroker>, enabled: bool) -> Self {
        Self {
            http,
            sockets: HashMap::new(),
            enabled,
        }
    }

    fn check(&mut self) -> Result<()> {
        if !self.enabled {
            return Err(error(
                "PERMISSION_DENIED",
                "Managed WebSocket feature was not negotiated",
            ));
        }
        let result = self.http.policy.check(self.http.policy.scope());
        if result.is_err() {
            self.sockets.clear();
        }
        result
    }

    pub(crate) async fn open(&mut self, args: &Value, control: &Control) -> Result<Value> {
        self.check()?;
        if self.sockets.len() >= limit("maxWebSocketConnections") {
            return Err(error(
                "BUDGET_EXCEEDED",
                "Invocation owns too many live WebSockets",
            ));
        }
        let request = &args["plan"];
        let max_bytes = request["maxMessageBytes"].as_u64().unwrap_or(0) as usize;
        if max_bytes == 0 || max_bytes > limit("maxWebSocketMessageBytes") {
            return Err(error(
                "INVALID_INPUT",
                "WebSocket message budget is invalid",
            ));
        }
        let raw = request["url"].as_str().unwrap_or_default();
        let url = websocket_http_url(raw)?;
        let plan = serde_json::from_value(json!({
            "method":"GET", "url":url.as_str(), "headers":request["headers"],
            "generatedHeaders":request["generatedHeaders"], "timeoutMs":request["timeoutMs"],
            "body":{"kind":"none"}, "sink":{"kind":"buffer","maxBytes":1},
        }))
        .map_err(|_| error("INVALID_INPUT", "WebSocket handshake request is invalid"))?;
        let key = generate_key();
        let opened = self
            .http
            .open_websocket_upgrade(&plan, args["credential"].as_str(), control, &key)
            .await?;
        verify_upgrade(&opened.response, &key)?;
        let upgraded = self
            .http
            .guarded(&opened.control, async {
                opened
                    .response
                    .upgrade()
                    .await
                    .map_err(|_| error("RESOURCE_UNAVAILABLE", "WebSocket upgrade failed"))
            })
            .await?;
        let config = WebSocketConfig::default()
            .read_buffer_size(max_bytes.min(8192))
            .write_buffer_size(0)
            .max_write_buffer_size(max_bytes + 128)
            .max_message_size(Some(max_bytes))
            .max_frame_size(Some(max_bytes));
        let stream = WebSocketStream::from_raw_socket(upgraded, Role::Client, Some(config)).await;
        let id = Uuid::new_v4().to_string();
        self.sockets
            .insert(id.clone(), Socket { stream, max_bytes });
        Ok(json!({"socketId": id}))
    }

    fn take(&mut self, id: &str) -> Result<Socket> {
        self.check()?;
        self.sockets.remove(id).ok_or_else(|| {
            error(
                "PERMISSION_DENIED",
                "WebSocket handle is not live in this invocation",
            )
        })
    }

    pub(crate) async fn read(
        &mut self,
        id: &str,
        wait_ms: u64,
        control: &Control,
    ) -> Result<Value> {
        let mut socket = self.take(id)?;
        let deadline = (wait_ms != 0).then(|| Instant::now() + Duration::from_millis(wait_ms));
        let message = self
            .http
            .guarded(control, async {
                loop {
                    let frame = if let Some(deadline) = deadline {
                        match timeout_at(deadline, socket.stream.next()).await {
                            Ok(frame) => frame,
                            Err(_) => return Ok(json!({"kind":"idle"})),
                        }
                    } else {
                        socket.stream.next().await
                    };
                    match frame {
                        Some(Ok(Message::Text(text))) => {
                            return Ok(json!({"kind":"text","text":text.as_str()}))
                        }
                        Some(Ok(Message::Binary(bytes))) => {
                            return Ok(json!({"kind":"binary","bytes":bytes.as_ref()}))
                        }
                        Some(Ok(Message::Close(_))) => match socket.stream.flush().await {
                            Ok(()) | Err(Error::ConnectionClosed) => {
                                return Ok(json!({"kind":"closed"}))
                            }
                            Err(error) => return Err(transport_error(error)),
                        },
                        Some(Ok(Message::Ping(_))) => {
                            socket.stream.flush().await.map_err(transport_error)?
                        }
                        Some(Ok(Message::Pong(_))) => {}
                        Some(Err(error)) => return Err(transport_error(error)),
                        None | Some(Ok(Message::Frame(_))) => {
                            return Err(error(
                                "RESOURCE_UNAVAILABLE",
                                "WebSocket closed without a valid close message",
                            ));
                        }
                    }
                }
            })
            .await?;
        if message["kind"] != "closed" {
            self.sockets.insert(id.to_owned(), socket);
        }
        Ok(message)
    }

    pub(crate) async fn write(
        &mut self,
        id: &str,
        message: &Value,
        control: &Control,
    ) -> Result<()> {
        self.check()?;
        let max_bytes = self
            .sockets
            .get(id)
            .ok_or_else(|| {
                error(
                    "PERMISSION_DENIED",
                    "WebSocket handle is not live in this invocation",
                )
            })?
            .max_bytes;
        let too_large = || {
            error(
                "BUDGET_EXCEEDED",
                "WebSocket write exceeds the negotiated byte limit",
            )
        };
        let frame = match message["kind"].as_str() {
            Some("text") => {
                let text = message["text"].as_str().unwrap_or_default();
                if text.len() > max_bytes {
                    return Err(too_large());
                }
                Message::Text(text.to_owned().into())
            }
            Some("binary") => {
                let items = message["bytes"]
                    .as_array()
                    .ok_or_else(|| error("INVALID_INPUT", "WebSocket binary message is invalid"))?;
                if items.len() > max_bytes {
                    return Err(too_large());
                }
                let bytes = serde_json::from_value::<Vec<u8>>(message["bytes"].clone())
                    .map_err(|_| error("INVALID_INPUT", "WebSocket binary message is invalid"))?;
                Message::Binary(bytes.into())
            }
            _ => {
                return Err(error(
                    "INVALID_INPUT",
                    "WebSocket writes require a text or binary message",
                ))
            }
        };
        let mut socket = self.take(id)?;
        self.http
            .guarded(control, async {
                socket.stream.send(frame).await.map_err(transport_error)
            })
            .await?;
        self.sockets.insert(id.to_owned(), socket);
        Ok(())
    }

    pub(crate) async fn close(&mut self, id: &str, control: &Control) -> Result<()> {
        let mut socket = self.take(id)?;
        self.http
            .guarded(control, async {
                socket.stream.close(None).await.map_err(transport_error)
            })
            .await
    }
}
