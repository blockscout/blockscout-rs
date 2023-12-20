use actix_web::{
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    http::StatusCode,
    Error, ResponseError,
};
use once_cell::sync::Lazy;
use std::{collections::HashMap, sync::Mutex, time::Instant};
use tracing::{Id, Span};
use tracing_actix_web::root_span;

static REQUEST_TIMINGS: Lazy<Mutex<HashMap<Id, Instant>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Default)]
pub struct CompactRootSpanBuilder;

impl tracing_actix_web::RootSpanBuilder for CompactRootSpanBuilder {
    fn on_request_start(request: &ServiceRequest) -> Span {
        let span = root_span!(
            request,
            duration = tracing::field::Empty,
            unit = tracing::field::Empty
        );
        // Will be none if tracing subscriber is not initialized
        if let Some(span_id) = span.id() {
            REQUEST_TIMINGS
                .lock()
                .unwrap()
                .insert(span_id, Instant::now());
        }
        span
    }

    fn on_request_end<B: MessageBody>(span: Span, outcome: &Result<ServiceResponse<B>, Error>) {
        // Will be none if tracing subscriber is not initialized
        if let Some(span_id) = span.id() {
            let start = REQUEST_TIMINGS.lock().unwrap().remove(&span_id);
            if let Some(start) = start {
                let duration = Instant::now() - start;
                span.record("duration", duration.as_micros());
                span.record("unit", "microsecond");
            }
        }

        match &outcome {
            Ok(response) => {
                if let Some(error) = response.response().error() {
                    // use the status code already constructed for the outgoing HTTP response
                    handle_error(span, response.status(), error.as_response_error());
                } else {
                    let code: i32 = response.response().status().as_u16().into();
                    span.record("http.status_code", code);
                }
            }
            Err(error) => {
                let response_error = error.as_response_error();
                handle_error(span, response_error.status_code(), response_error);
            }
        };
    }
}

fn handle_error(span: Span, status_code: StatusCode, response_error: &dyn ResponseError) {
    // pre-formatting errors is a workaround for https://github.com/tokio-rs/tracing/issues/1565
    let display = format!("{response_error}");
    let debug = format!("{response_error:?}");
    span.record("exception.message", &tracing::field::display(display));
    span.record("exception.details", &tracing::field::display(debug));
    let code: i32 = status_code.as_u16().into();

    span.record("http.status_code", code);
}
