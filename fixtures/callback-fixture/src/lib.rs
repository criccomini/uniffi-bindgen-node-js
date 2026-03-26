#[uniffi::export(callback_interface)]
pub trait LogSink: Send + Sync {
    fn write(&self, message: String);
    fn latest(&self) -> Option<String>;
}

#[uniffi::export]
pub fn emit(sink: Box<dyn LogSink>, message: String) {
    sink.write(message);
}

#[uniffi::export]
pub fn last_message(sink: Option<Box<dyn LogSink>>) -> Option<String> {
    sink.and_then(|sink| sink.latest())
}

uniffi::setup_scaffolding!("callbacks_fixture");
