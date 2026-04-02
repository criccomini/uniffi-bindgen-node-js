use thiserror::Error;

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    fast,
    slow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Ok { value: String },
    Err,
}

#[derive(Debug, Error)]
pub enum DocsError {
    #[error("{message}")]
    Invalid { message: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Payload {
    pub label: String,
}

pub trait Reporter: Send + Sync {
    fn report(&self, message: String);
}

#[derive(Debug)]
pub struct Greeter {
    label: String,
}

pub fn echo(value: String) -> Result<String, DocsError> {
    if value.is_empty() {
        Err(DocsError::Invalid {
            message: "value must not be empty".to_string(),
        })
    } else {
        Ok(value)
    }
}

impl Greeter {
    pub fn new(payload: Payload) -> Self {
        Self { label: payload.label }
    }

    pub fn from_label(label: String) -> Self {
        Self { label }
    }

    pub fn greet(&self) -> String {
        format!("Hello, {}!", self.label)
    }
}

uniffi::include_scaffolding!("docs_fixture");
