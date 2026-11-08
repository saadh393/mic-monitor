use std::time::Duration;

pub const SILENCE_LIMIT: Duration = Duration::from_secs(3);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Check {
    Waiting,
    Heard,
    Missing,
}

pub fn check(previous: Check, samples: &[f32], elapsed: Duration) -> Check {
    if previous != Check::Waiting {
        return previous;
    }
    if samples.iter().any(|&sample| sample != 0.0) {
        return Check::Heard;
    }
    if elapsed >= SILENCE_LIMIT {
        return Check::Missing;
    }
    Check::Waiting
}

#[cfg(test)]
mod tests {
    use super::{check, Check};
    use std::time::Duration;

    fn secs(s: f64) -> Duration {
        Duration::from_secs_f64(s)
    }

    #[test]
    fn waits_while_silent_under_3s() {
        assert_eq!(check(Check::Waiting, &[0.0; 512], secs(2.9)), Check::Waiting);
    }

    #[test]
    fn missing_after_3s_of_silence() {
        assert_eq!(check(Check::Waiting, &[0.0; 512], secs(3.0)), Check::Missing);
    }

    #[test]
    fn empty_chunk_counts_as_silence() {
        assert_eq!(check(Check::Waiting, &[], secs(3.0)), Check::Missing);
        assert_eq!(check(Check::Waiting, &[], secs(1.0)), Check::Waiting);
    }

    #[test]
    fn heard_on_any_nonzero_sample() {
        assert_eq!(check(Check::Waiting, &[0.0, 0.0001, 0.0], secs(1.0)), Check::Heard);
        assert_eq!(check(Check::Waiting, &[0.0, 0.0001, 0.0], secs(3.0)), Check::Heard);
    }

    #[test]
    fn negative_zero_is_silence() {
        assert_eq!(check(Check::Waiting, &[-0.0, 0.0], secs(3.0)), Check::Missing);
    }

    #[test]
    fn verdict_is_final() {
        assert_eq!(check(Check::Heard, &[0.0], secs(10.0)), Check::Heard);
        assert_eq!(check(Check::Missing, &[0.5], secs(10.0)), Check::Missing);
    }
}
