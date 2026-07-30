//! `ChatGPT Codex` HTTP wrapper for request fields `Rig` intentionally clears.

use bytes::Bytes;
use rig_core::http_client::{
    HttpClientExt, LazyBody, MultipartForm, Request, Response, Result, StreamingResponse,
};
use serde_json::Value;
use std::future::Future;

/// Injects `ChatGPT` Fast mode after `Rig` builds its `Codex` request.
///
/// `Rig` 0.39 clears `service_tier` in the `ChatGPT` adapter. `OpenFlow` keeps using
/// `Rig` for request construction and streaming, then restores this one
/// user-selected field at the HTTP boundary.
#[derive(Clone, Debug, Default)]
#[allow(clippy::redundant_pub_crate)] // Exposed through the crate-visible RigModel enum.
pub(crate) struct CodexHttpClient {
    inner: reqwest::Client,
    fast_mode: bool,
}

impl CodexHttpClient {
    pub(crate) const fn new(inner: reqwest::Client, fast_mode: bool) -> Self {
        Self { inner, fast_mode }
    }
}

impl HttpClientExt for CodexHttpClient {
    fn send<T, U>(
        &self,
        request: Request<T>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + Send + 'static
    where
        T: Into<Bytes> + Send,
        U: From<Bytes> + Send + 'static,
    {
        self.inner
            .send::<Bytes, U>(with_fast_service_tier(request, self.fast_mode))
    }

    fn send_multipart<U>(
        &self,
        request: Request<MultipartForm>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + Send + 'static
    where
        U: From<Bytes> + Send + 'static,
    {
        self.inner.send_multipart(request)
    }

    fn send_streaming<T>(
        &self,
        request: Request<T>,
    ) -> impl Future<Output = Result<StreamingResponse>> + Send
    where
        T: Into<Bytes> + Send,
    {
        self.inner
            .send_streaming(with_fast_service_tier(request, self.fast_mode))
    }
}

fn with_fast_service_tier<T: Into<Bytes>>(request: Request<T>, fast_mode: bool) -> Request<Bytes> {
    request.map(|body| {
        let bytes = body.into();
        if fast_mode {
            inject_fast_service_tier(bytes)
        } else {
            bytes
        }
    })
}

fn inject_fast_service_tier(body: Bytes) -> Bytes {
    let Ok(mut value) = serde_json::from_slice::<Value>(&body) else {
        return body;
    };
    let Some(object) = value.as_object_mut() else {
        return body;
    };
    // ChatGPT calls this UI mode Fast. Its request contract names the tier
    // `priority`; `fast` is a Codex config value, not a service tier value.
    object.insert(
        "service_tier".to_string(),
        Value::String("priority".to_string()),
    );
    serde_json::to_vec(&value).map_or(body, Bytes::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fast_mode_injects_service_tier_without_changing_reasoning() {
        let body = Bytes::from(
            json!({
                "model": "gpt-5.4",
                "reasoning": { "effort": "high" }
            })
            .to_string(),
        );

        let injected = inject_fast_service_tier(body);
        let value = serde_json::from_slice::<Value>(&injected).unwrap_or_default();

        assert_eq!(value["service_tier"], "priority");
        assert_eq!(value["reasoning"]["effort"], "high");
    }
}
