use http::Request;
use http_body::Body;
use log::debug;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::Service;

#[derive(Clone)]
pub struct UriTracingLayer;

impl<S> tower::Layer<S> for UriTracingLayer {
    type Service = UriTracingMiddleware<S>;
    fn layer(&self, inner: S) -> Self::Service {
        UriTracingMiddleware { inner }
    }
}

#[derive(Clone)]
pub struct UriTracingMiddleware<S> {
    inner: S,
}

impl<S, B> Service<Request<B>> for UriTracingMiddleware<S>
where
    S: Service<Request<B>> + Clone + Send + 'static,
    S::Response: 'static,
    S::Future: Send + 'static,
    S::Error: std::fmt::Debug,
    B: Body + Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        let uri = req.uri().clone();
        debug!("gRPC call to URI: {}", uri);
        let fut = self.inner.call(req);
        Box::pin(fut)
    }
}
