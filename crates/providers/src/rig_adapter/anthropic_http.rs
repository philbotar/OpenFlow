//! Compatibility HTTP client for Anthropic-shaped provider responses.

use bytes::Bytes;
use rig_core::http_client::{
    HttpClientExt, LazyBody, MultipartForm, Request, Response, Result, StreamingResponse,
};
use std::future::Future;

/// Rig models Anthropic response collections as non-null arrays, while some
/// compatible gateways serialize absent collections as `null`. Normalize the
/// two response fields we consume before Rig deserializes the message.
#[derive(Clone, Default)]
pub(crate) struct AnthropicHttpClient {
    inner: reqwest::Client,
}

impl AnthropicHttpClient {
    pub(crate) const fn new(inner: reqwest::Client) -> Self {
        Self { inner }
    }
}

impl HttpClientExt for AnthropicHttpClient {
    fn send<T, U>(
        &self,
        request: Request<T>,
    ) -> impl Future<Output = Result<Response<LazyBody<U>>>> + Send + 'static
    where
        T: Into<Bytes> + Send,
        U: From<Bytes> + Send + 'static,
    {
        let response = self.inner.send::<T, Bytes>(request);
        async move {
            let response = response.await?;
            let (parts, body) = response.into_parts();
            let normalized: LazyBody<U> = Box::pin(async move {
                let body = body.await?;
                Ok(U::from(normalize_response(body)))
            });
            Ok(Response::from_parts(parts, normalized))
        }
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
        self.inner.send_streaming(request)
    }
}

fn normalize_response(body: Bytes) -> Bytes {
    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(&body) else {
        return body;
    };
    let Some(message) = value.as_object_mut() else {
        return body;
    };
    if message.get("type").and_then(serde_json::Value::as_str) != Some("message") {
        return body;
    }

    let mut changed = if message
        .get("content")
        .is_some_and(serde_json::Value::is_null)
    {
        message.insert("content".into(), serde_json::Value::Array(Vec::new()));
        true
    } else {
        false
    };
    if let Some(content) = message
        .get_mut("content")
        .and_then(serde_json::Value::as_array_mut)
    {
        for block in content {
            let Some(block) = block.as_object_mut() else {
                continue;
            };
            if block
                .get("citations")
                .is_some_and(serde_json::Value::is_null)
            {
                block.insert("citations".into(), serde_json::Value::Array(Vec::new()));
                changed = true;
            }
        }
    }

    if !changed {
        return body;
    }
    serde_json::to_vec(&value).map_or(body, Bytes::from)
}
