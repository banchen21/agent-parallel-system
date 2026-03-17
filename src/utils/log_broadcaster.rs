use std::sync::OnceLock;
use tokio::sync::broadcast;
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::Context;

/// 单条日志条目，序列化后通过 SSE 推送到前端。
#[derive(Debug, Clone, serde::Serialize)]
pub struct LogEntry {
    pub time: String,
    pub level: String,
    pub message: String,
    pub target: String,
}

static LOG_TX: OnceLock<broadcast::Sender<LogEntry>> = OnceLock::new();

/// 在 main() 最开始调用，初始化广播通道。
pub fn init_broadcaster() {
    let (tx, _) = broadcast::channel(2048);
    let _ = LOG_TX.set(tx);
}

pub fn get_log_sender() -> Option<&'static broadcast::Sender<LogEntry>> {
    LOG_TX.get()
}

// ─── tracing Layer ────────────────────────────────────────────────

struct MessageVisitor {
    message: String,
}

impl Visit for MessageVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_string();
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = format!("{:?}", value);
        }
    }
}

/// 将 tracing 事件广播到 LOG_TX。
pub struct BroadcastLayer;

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for BroadcastLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let level = match *event.metadata().level() {
            tracing::Level::ERROR => "error",
            tracing::Level::WARN => "warn",
            tracing::Level::INFO => "info",
            tracing::Level::DEBUG => "debug",
            tracing::Level::TRACE => "trace",
        };

        let mut visitor = MessageVisitor {
            message: String::new(),
        };
        event.record(&mut visitor);

        if let Some(tx) = LOG_TX.get() {
            let entry = LogEntry {
                time: chrono::Local::now()
                    .format("%H:%M:%S%.3f")
                    .to_string(),
                level: level.to_string(),
                message: visitor.message,
                target: event.metadata().target().to_string(),
            };
            let _ = tx.send(entry);
        }
    }
}
