use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct LogRecord {
    pub level: LogLevel,
    pub target: String,
    pub message: String,
    pub module_path: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
}

#[uniffi::export(callback_interface)]
pub trait LogSink: Send + Sync {
    fn write(&self, message: String);
    fn latest(&self) -> Option<String>;
}

#[uniffi::export(callback_interface)]
pub trait LogCollector: Send + Sync {
    fn log(&self, record: LogRecord);
}

#[derive(Debug, uniffi::Object)]
pub struct Settings {
    values: Mutex<JsonObject>,
}

#[derive(Debug, uniffi::Object)]
pub struct WriteBatch {
    operations: Mutex<Vec<BatchOperation>>,
}

type JsonObject = BTreeMap<String, JsonNode>;

#[derive(Clone, Debug, PartialEq, Eq)]
enum JsonNode {
    Object(JsonObject),
    Raw(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum BatchOperation {
    Put { key: Vec<u8>, value: Vec<u8> },
    Delete { key: Vec<u8> },
}

#[uniffi::export]
pub fn emit(sink: Box<dyn LogSink>, message: String) {
    sink.write(message);
}

#[uniffi::export]
pub fn last_message(sink: Option<Box<dyn LogSink>>) -> Option<String> {
    sink.and_then(|sink| sink.latest())
}

#[uniffi::export]
pub fn init_logging(level: LogLevel, collector: Option<Box<dyn LogCollector>>) {
    if let Some(collector) = collector {
        collector.log(LogRecord {
            level,
            target: "callbacks_fixture".to_string(),
            message: "logging initialized".to_string(),
            module_path: Some("callbacks_fixture::logging".to_string()),
            file: None,
            line: None,
        });
    }
}

#[uniffi::export]
impl Settings {
    #[uniffi::constructor(name = "default")]
    pub fn default() -> Arc<Self> {
        Arc::new(Self {
            values: Mutex::new(BTreeMap::new()),
        })
    }

    pub fn set(&self, key: String, value_json: String) {
        insert_json_value(&mut self.values.lock().unwrap(), &key, &value_json);
    }

    pub fn to_json_string(&self) -> String {
        render_json_object(&self.values.lock().unwrap())
    }
}

#[uniffi::export]
impl WriteBatch {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            operations: Mutex::new(Vec::new()),
        })
    }

    pub fn put(&self, key: Vec<u8>, value: Vec<u8>) {
        self.operations
            .lock()
            .unwrap()
            .push(BatchOperation::Put { key, value });
    }

    pub fn delete(&self, key: Vec<u8>) {
        self.operations
            .lock()
            .unwrap()
            .push(BatchOperation::Delete { key });
    }

    pub fn operation_count(&self) -> u32 {
        self.operations.lock().unwrap().len() as u32
    }
}

fn insert_json_value(root: &mut JsonObject, key: &str, value_json: &str) {
    let path: Vec<&str> = key.split('.').collect();
    if path.is_empty() {
        return;
    }
    insert_json_value_at_path(root, &path, value_json);
}

fn insert_json_value_at_path(root: &mut JsonObject, path: &[&str], value_json: &str) {
    if path.len() == 1 {
        root.insert(path[0].to_string(), JsonNode::Raw(value_json.to_string()));
        return;
    }

    let child = root
        .entry(path[0].to_string())
        .or_insert_with(|| JsonNode::Object(BTreeMap::new()));
    let child_object = match child {
        JsonNode::Object(object) => object,
        JsonNode::Raw(_) => {
            *child = JsonNode::Object(BTreeMap::new());
            match child {
                JsonNode::Object(object) => object,
                JsonNode::Raw(_) => unreachable!("node was just converted into an object"),
            }
        }
    };

    insert_json_value_at_path(child_object, &path[1..], value_json);
}

fn render_json_object(object: &JsonObject) -> String {
    let items = object
        .iter()
        .map(|(key, value)| format!("{}:{}", render_json_key(key), render_json_node(value)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{items}}}")
}

fn render_json_node(node: &JsonNode) -> String {
    match node {
        JsonNode::Object(object) => render_json_object(object),
        JsonNode::Raw(value) => value.clone(),
    }
}

fn render_json_key(key: &str) -> String {
    let escaped: String = key.chars().flat_map(char::escape_default).collect();
    format!("\"{escaped}\"")
}

uniffi::setup_scaffolding!("callbacks_fixture");
