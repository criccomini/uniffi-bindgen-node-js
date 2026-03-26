use std::sync::{Arc, Mutex};

use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct BlobRecord {
    pub name: String,
    pub value: Vec<u8>,
    pub maybe_value: Option<Vec<u8>>,
    pub chunks: Vec<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Flavor {
    Vanilla,
    Chocolate,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ScanResult {
    Hit { value: Vec<u8> },
    Miss,
}

#[derive(Debug, Error, uniffi::Error)]
pub enum FixtureError {
    #[error("missing value")]
    Missing,
    #[error("invalid state: {message}")]
    InvalidState { message: String },
}

#[derive(Debug, uniffi::Object)]
pub struct Store {
    state: Mutex<BlobRecord>,
}

#[uniffi::export]
pub fn echo_record(record: BlobRecord) -> BlobRecord {
    record
}

#[uniffi::export]
pub fn echo_bytes(value: Vec<u8>) -> Vec<u8> {
    value
}

#[uniffi::export(async_runtime = "tokio")]
impl Store {
    #[uniffi::constructor]
    pub fn new(seed: BlobRecord) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(seed),
        })
    }

    pub fn current(&self) -> BlobRecord {
        self.state.lock().unwrap().clone()
    }

    pub fn replace(&self, next_value: Vec<u8>) -> Vec<u8> {
        let mut state = self.state.lock().unwrap();
        let previous = state.value.clone();
        state.value = next_value.clone();
        state.maybe_value = Some(next_value.clone());
        state.chunks.push(next_value);
        previous
    }

    pub fn flavor(&self) -> Flavor {
        if self.state.lock().unwrap().value.len() % 2 == 0 {
            Flavor::Vanilla
        } else {
            Flavor::Chocolate
        }
    }

    pub fn inspect(&self, include_payload: bool) -> ScanResult {
        if include_payload {
            ScanResult::Hit {
                value: self.state.lock().unwrap().value.clone(),
            }
        } else {
            ScanResult::Miss
        }
    }

    pub fn require_value(&self, present: bool) -> Result<Vec<u8>, FixtureError> {
        if present {
            Ok(self.state.lock().unwrap().value.clone())
        } else {
            Err(FixtureError::Missing)
        }
    }

    pub async fn fetch_async(&self, succeed: bool) -> Result<BlobRecord, FixtureError> {
        if succeed {
            Ok(self.current())
        } else {
            Err(FixtureError::InvalidState {
                message: "fetch failed".to_string(),
            })
        }
    }
}

uniffi::setup_scaffolding!("fixture");
