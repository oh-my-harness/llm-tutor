use serde::Serialize;
use tokio::sync::broadcast;

/// Events pushed from the agent harness to the WebSocket handler.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload")]
#[serde(rename_all = "snake_case")]
pub enum StreamEvent {
    /// LLM text chunk or final message.
    Content { text: String, chunk: bool },
    /// Internal event for the TracePanel.
    Trace {
        kind: String,
        #[serde(flatten)]
        data: serde_json::Value,
    },
    /// Status notification.
    Status {
        kind: String,
        #[serde(flatten)]
        data: serde_json::Value,
    },
}

/// WebSocket event bus — one per active session.
#[derive(Clone)]
pub struct TutorStream {
    tx: broadcast::Sender<StreamEvent>,
}

impl TutorStream {
    pub fn new(capacity: usize) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<StreamEvent> {
        self.tx.subscribe()
    }

    pub async fn content(&self, text: &str, chunk: bool) {
        let _ = self.tx.send(StreamEvent::Content {
            text: text.to_string(),
            chunk,
        });
    }

    pub async fn trace(&self, kind: &str, data: impl Serialize) {
        let _ = self.tx.send(StreamEvent::Trace {
            kind: kind.to_string(),
            data: serde_json::to_value(data).unwrap_or_default(),
        });
    }

    pub async fn status(&self, kind: &str, data: impl Serialize) {
        let _ = self.tx.send(StreamEvent::Status {
            kind: kind.to_string(),
            data: serde_json::to_value(data).unwrap_or_default(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_event_serializes_to_json() {
        let event = StreamEvent::Content {
            text: "hello".into(),
            chunk: true,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "content");
        assert_eq!(json["payload"]["text"], "hello");
        assert_eq!(json["payload"]["chunk"], true);
    }

    #[test]
    fn trace_event_serializes_correctly() {
        let event = StreamEvent::Trace {
            kind: "phase_start".into(),
            data: serde_json::json!({ "phase": "plan" }),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "trace");
        assert_eq!(json["payload"]["kind"], "phase_start");
        assert_eq!(json["payload"]["phase"], "plan");
    }

    #[tokio::test]
    async fn tutor_stream_sends_content() {
        let stream = TutorStream::new(16);
        let mut rx = stream.subscribe();
        stream.content("hello", true).await;
        let event = rx.recv().await.unwrap();
        match event {
            StreamEvent::Content { text, chunk } => {
                assert_eq!(text, "hello");
                assert!(chunk);
            }
            _ => panic!("expected Content"),
        }
    }
}
