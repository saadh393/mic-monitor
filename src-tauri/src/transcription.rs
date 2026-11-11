use std::time::Duration;

pub const DEFAULT_CHUNK_SECONDS: u64 = 8 * 60;
pub const DEFAULT_OVERLAP_SECONDS: u64 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk {
    pub index: u32,
    pub offset_seconds: u64,
    pub duration_seconds: u64,
}

pub fn plan(duration_seconds: u64, chunk_seconds: u64, overlap_seconds: u64) -> Vec<Chunk> {
    if duration_seconds == 0 || chunk_seconds == 0 || overlap_seconds >= chunk_seconds {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut offset = 0;
    let mut index = 0;
    while offset < duration_seconds {
        let end = (offset + chunk_seconds).min(duration_seconds);
        chunks.push(Chunk {
            index,
            offset_seconds: offset,
            duration_seconds: end - offset,
        });
        if end == duration_seconds {
            break;
        }
        offset = end - overlap_seconds;
        index += 1;
    }
    chunks
}

pub fn retry_delay(attempt: u32, server_delay: Option<Duration>) -> Duration {
    if let Some(delay) = server_delay {
        return delay;
    }
    let exponent = attempt.min(6);
    Duration::from_secs(2u64.saturating_pow(exponent).min(60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_overlapping_chunks_without_empty_tail() {
        let chunks = plan(1_000, 400, 10);
        assert_eq!(chunks[0].offset_seconds, 0);
        assert_eq!(chunks[1].offset_seconds, 390);
        assert_eq!(chunks.last().unwrap().duration_seconds, 220);
    }

    #[test]
    fn rejects_invalid_chunk_sizes() {
        assert!(plan(100, 0, 1).is_empty());
        assert!(plan(100, 10, 10).is_empty());
    }

    #[test]
    fn retry_prefers_server_delay_and_caps_exponential_delay() {
        assert_eq!(
            retry_delay(2, Some(Duration::from_secs(17))),
            Duration::from_secs(17)
        );
        assert_eq!(retry_delay(20, None), Duration::from_secs(60));
    }
}
