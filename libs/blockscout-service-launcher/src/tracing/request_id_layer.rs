use std::{fmt::Debug, str::FromStr};
use tracing::{field::Field, span::Attributes, Id, Subscriber};
use tracing_subscriber::{
    fmt::{format::JsonFields, FormattedFields},
    layer::Context,
    registry::LookupSpan,
    Layer,
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
struct RequestId(Uuid);

pub fn layer() -> RequestIdStorage {
    RequestIdStorage
}

pub struct RequestIdStorage;

impl<S: Subscriber + for<'lookup> LookupSpan<'lookup>> Layer<S> for RequestIdStorage {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("Span not found, this is a bug");

        let request_id = if let Some(parent_span) = span.parent() {
            let request_id = parent_span.extensions().get::<RequestId>().cloned();

            let mut extensions = span.extensions_mut();
            if extensions
                .get_mut::<FormattedFields<JsonFields>>()
                .is_none()
            {
                extensions.insert(FormattedFields::<JsonFields>::new("{}".to_string()));
            }
            let data = extensions.get_mut::<FormattedFields<JsonFields>>().unwrap();
            if let Some(request_id) = request_id {
                match serde_json::from_str::<serde_json::Value>(data) {
                    Ok(serde_json::Value::Object(mut fields))
                        if !fields.contains_key("request_id") =>
                    {
                        fields.insert("request_id".into(), request_id.0.to_string().into());
                        data.fields = serde_json::Value::Object(fields).to_string();
                    }
                    // If the value is not found or has invalid type, just ignore the error
                    // and propagate it further. Default Format<Json> layer will handle those cases.
                    _ => {}
                };
            };
            request_id
        } else if let Some(field) = attrs.fields().field("request_id") {
            struct Visitor {
                request_id_field: Field,
                request_id_value: Option<RequestId>,
            }
            impl tracing::field::Visit for Visitor {
                fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
                    if field == &self.request_id_field {
                        if let Ok(request_id) = Uuid::from_str(&format!("{value:?}")) {
                            self.request_id_value = Some(RequestId(request_id));
                        }
                    }
                }
            }

            let mut visitor = Visitor {
                request_id_field: field,
                request_id_value: None,
            };
            attrs.record(&mut visitor);

            visitor.request_id_value
        } else {
            None
        };

        if let Some(request_id) = request_id {
            span.extensions_mut().insert(request_id);
        }
    }
}
