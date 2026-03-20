use axum::http::{HeaderValue, Request};
use tower::Layer;
use tower::Service;
use uuid::Uuid;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

const REQUEST_ID_HEADER: &str = "x-request-id";

#[derive(Clone)]
pub struct RequestIdLayer;

pub fn layer() -> RequestIdLayer {
    RequestIdLayer
}

impl<S> Layer<S> for RequestIdLayer {
    type Service = RequestIdService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestIdService { inner }
    }
}

#[derive(Clone)]
pub struct RequestIdService<S> {
    inner: S,
}

impl<S, B> Service<Request<B>> for RequestIdService<S>
where
    S: Service<Request<B>, Response = axum::response::Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    B: Send + 'static,
{
    type Response = axum::response::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        let request_id = req
            .headers()
            .get(REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        if let Ok(val) = HeaderValue::from_str(&request_id) {
            req.headers_mut().insert(REQUEST_ID_HEADER, val);
        }

        let mut inner = self.inner.clone();
        Box::pin(async move {
            let mut response = inner.call(req).await?;
            if let Ok(val) = HeaderValue::from_str(&request_id) {
                response.headers_mut().insert(REQUEST_ID_HEADER, val);
            }
            Ok(response)
        })
    }
}
