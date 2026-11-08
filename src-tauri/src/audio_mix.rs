pub const RATE: u32 = 48_000;

fn to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
    let channels = channels.max(1) as usize;
    samples.chunks_exact(channels).map(|frame| frame.iter().sum::<f32>() / channels as f32).collect()
}

fn resample(mono: &[f32], rate: u32) -> Vec<f32> {
    if rate == RATE || mono.is_empty() {
        return mono.to_vec();
    }
    let step = rate as f64 / RATE as f64;
    let len = (mono.len() as f64 / step).round() as usize;
    (0..len)
        .map(|i| {
            let position = i as f64 * step;
            let index = (position as usize).min(mono.len() - 1);
            let next = mono[(index + 1).min(mono.len() - 1)];
            mono[index] + (next - mono[index]) * (position - index as f64) as f32
        })
        .collect()
}

pub fn convert(samples: &[f32], rate: u32, channels: u16) -> Vec<f32> {
    resample(&to_mono(samples, channels), rate)
}

pub fn mix(mic: &[f32], meeting: &[f32]) -> Vec<f32> {
    (0..mic.len().max(meeting.len())).map(|i| (mic.get(i).unwrap_or(&0.0) + meeting.get(i).unwrap_or(&0.0)).clamp(-1.0, 1.0)).collect()
}

pub fn mixable(mic: usize, meeting: Option<usize>) -> usize {
    match meeting {
        None => mic,
        Some(meeting) if mic.abs_diff(meeting) > RATE as usize => mic.max(meeting),
        Some(meeting) => mic.min(meeting),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_and_clamps() {
        assert_eq!(mix(&[0.5, 0.75, -0.75], &[0.25, 0.5, -0.5]), vec![0.75, 1.0, -1.0]);
    }

    #[test]
    fn pads_shorter_stream() {
        assert_eq!(mix(&[0.5, 0.5], &[0.25]), vec![0.75, 0.5]);
    }

    #[test]
    fn converts_to_48k_mono() {
        let stereo: Vec<f32> = (0..480).flat_map(|_| [1.0, 0.0]).collect();
        assert_eq!(convert(&stereo, 48_000, 2), vec![0.5; 480]);
        assert!(convert(&vec![0.25; 441], 44_100, 1).len().abs_diff(480) <= 1);
    }

    #[test]
    fn waits_for_both_streams() {
        assert_eq!(mixable(4800, Some(2400)), 2400);
    }

    #[test]
    fn pads_stalled_stream_after_1s() {
        assert_eq!(mixable(48001 + 100, Some(100)), 48101);
        assert_eq!(mixable(100, Some(48001 + 100)), 48101);
    }

    #[test]
    fn mic_only_without_meeting_stream() {
        assert_eq!(mixable(4800, None), 4800);
    }
}
