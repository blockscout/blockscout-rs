use crate::proto;
use amplify::{From, Wrapper};
use eth_bytecode_db::search;

#[derive(Wrapper, From, Clone, Debug, PartialEq)]
pub struct EventDescriptionWrapper(proto::search_event_descriptions_response::EventDescription);

impl From<search::EventDescription> for EventDescriptionWrapper {
    fn from(value: search::EventDescription) -> Self {
        EventDescriptionWrapper(
            proto::search_event_descriptions_response::EventDescription {
                r#type: "event".to_string(),
                name: value.name,
                inputs: value.inputs.to_string(),
            },
        )
    }
}
