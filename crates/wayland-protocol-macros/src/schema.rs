use serde::Deserialize;

#[derive(Deserialize)]
pub struct Protocol {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
    pub copyright: String,
    pub interface: Vec<Interface>,
}

#[derive(Deserialize)]
pub struct Interface {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@version")]
    pub version: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
    pub description: InterfaceDescription,
    pub request: Option<Vec<Request>>,
    pub event: Option<Vec<Event>>,
    #[serde(rename = "enum")]
    pub interface_enum: Option<Vec<Enum>>,
}

#[derive(Deserialize)]
pub struct InterfaceDescription {
    #[serde(rename = "@summary")]
    pub summary: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
}

#[derive(Deserialize)]
pub struct Request {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@type")]
    pub request_type: Option<String>,
    #[serde(rename = "@since")]
    pub since: Option<String>,
    #[serde(rename = "$text")]
    pub text: Option<String>,
    pub description: RequestDescription,
    pub arg: Option<Vec<RequestArg>>,
}

#[derive(Deserialize)]
pub struct RequestDescription {
    #[serde(rename = "@summary")]
    pub summary: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
}

#[derive(Deserialize)]
pub struct RequestArg {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@type")]
    pub arg_type: String,
    #[serde(rename = "@interface")]
    pub interface: Option<String>,
    #[serde(rename = "@summary")]
    pub summary: String,
    #[serde(rename = "@enum")]
    pub arg_enum: Option<String>,
    #[serde(rename = "@allow-null")]
    pub allow_null: Option<bool>,
}

#[derive(Deserialize)]
pub struct Event {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@type")]
    pub event_type: Option<String>,
    #[serde(rename = "@since")]
    pub since: Option<String>,
    #[serde(rename = "@deprecated-since")]
    pub deprecated_since: Option<String>,
    #[serde(rename = "$text")]
    pub text: Option<String>,
    pub description: EventDescription,
    pub arg: Option<Vec<EventArg>>,
}

#[derive(Deserialize)]
pub struct EventDescription {
    #[serde(rename = "@summary")]
    pub summary: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
}

#[derive(Deserialize)]
pub struct EventArg {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@type")]
    pub arg_type: String,
    #[serde(rename = "@summary")]
    pub summary: String,
    #[serde(rename = "@enum")]
    pub arg_enum: Option<String>,
    #[serde(rename = "@allow-null")]
    pub allow_null: Option<bool>,
    #[serde(rename = "@interface")]
    pub interface: Option<String>,
}

#[derive(Deserialize)]
pub struct Enum {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@since")]
    pub since: Option<String>,
    #[serde(rename = "@bitfield")]
    pub bitfield: Option<String>,
    #[serde(rename = "$text")]
    pub text: Option<String>,
    pub description: Option<EnumDescription>,
    pub entry: Vec<Entry>,
}

#[derive(Deserialize)]
pub struct EnumDescription {
    #[serde(rename = "@summary")]
    pub summary: String,
    #[serde(rename = "$text")]
    pub text: Option<String>,
}

#[derive(Deserialize)]
pub struct Entry {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@value")]
    pub value: String,
    #[serde(rename = "@summary")]
    pub summary: Option<String>,
    #[serde(rename = "@since")]
    pub since: Option<String>,
}
