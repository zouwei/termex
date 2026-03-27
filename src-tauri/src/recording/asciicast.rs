use serde::{Deserialize, Serialize};

/// Asciicast v2 header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsciicastHeader {
    pub version: u32,
    pub width: u32,
    pub height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<serde_json::Value>,
}

impl AsciicastHeader {
    /// Creates a new v2 header.
    pub fn new(width: u32, height: u32, title: Option<String>) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .ok();

        Self {
            version: 2,
            width,
            height,
            timestamp,
            title,
            env: None,
        }
    }
}

/// A single event in an asciicast recording.
/// Format: `[time, event_type, data]`
/// time: seconds since recording start (f64)
/// event_type: "o" for output, "i" for input
/// data: the text data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsciicastEvent(pub f64, pub String, pub String);

impl AsciicastEvent {
    /// Creates an output event.
    pub fn output(time: f64, data: &str) -> Self {
        Self(time, "o".into(), data.to_string())
    }

    /// Creates an input event.
    pub fn input(time: f64, data: &str) -> Self {
        Self(time, "i".into(), data.to_string())
    }
}

/// Parsed asciicast file (header + events).
#[derive(Debug)]
pub struct AsciicastFile {
    pub header: AsciicastHeader,
    pub events: Vec<AsciicastEvent>,
}

impl AsciicastFile {
    /// Parses an asciicast v2 file from lines.
    pub fn parse(content: &str) -> Result<Self, super::RecordingError> {
        let mut lines = content.lines();
        let header_line = lines.next().ok_or_else(|| {
            super::RecordingError::Json(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "empty file",
            )))
        })?;
        let header: AsciicastHeader = serde_json::from_str(header_line)?;

        let mut events = Vec::new();
        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let event: AsciicastEvent = serde_json::from_str(line)?;
            events.push(event);
        }

        Ok(Self { header, events })
    }

    /// Serializes to asciicast v2 format (one JSON object per line).
    pub fn serialize(&self) -> Result<String, super::RecordingError> {
        let mut output = serde_json::to_string(&self.header)?;
        output.push('\n');
        for event in &self.events {
            output.push_str(&serde_json::to_string(event)?);
            output.push('\n');
        }
        Ok(output)
    }

    /// Returns the total duration of the recording in seconds.
    pub fn duration(&self) -> f64 {
        self.events.last().map(|e| e.0).unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_serialize() {
        let header = AsciicastHeader::new(80, 24, Some("test".into()));
        let json = serde_json::to_string(&header).unwrap();
        assert!(json.contains("\"version\":2"));
        assert!(json.contains("\"width\":80"));
        assert!(json.contains("\"title\":\"test\""));
    }

    #[test]
    fn test_event_serialize() {
        let event = AsciicastEvent::output(1.5, "hello world");
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, "[1.5,\"o\",\"hello world\"]");
    }

    #[test]
    fn test_parse_roundtrip() {
        let file = AsciicastFile {
            header: AsciicastHeader {
                version: 2,
                width: 80,
                height: 24,
                timestamp: Some(1700000000),
                title: Some("test session".into()),
                env: None,
            },
            events: vec![
                AsciicastEvent::output(0.0, "$ "),
                AsciicastEvent::input(0.5, "ls\r"),
                AsciicastEvent::output(0.6, "file1.txt  file2.txt\r\n$ "),
            ],
        };

        let serialized = file.serialize().unwrap();
        let parsed = AsciicastFile::parse(&serialized).unwrap();

        assert_eq!(parsed.header.version, 2);
        assert_eq!(parsed.header.width, 80);
        assert_eq!(parsed.events.len(), 3);
        assert!((parsed.duration() - 0.6).abs() < 0.001);
    }

    #[test]
    fn test_duration() {
        let file = AsciicastFile {
            header: AsciicastHeader::new(80, 24, None),
            events: vec![
                AsciicastEvent::output(0.0, "start"),
                AsciicastEvent::output(5.5, "end"),
            ],
        };
        assert!((file.duration() - 5.5).abs() < 0.001);
    }
}
