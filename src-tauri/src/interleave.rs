pub fn interleave(buffers: &[&[f32]]) -> Vec<f32> {
    let frames = buffers.iter().map(|buffer| buffer.len()).min().unwrap_or(0);
    (0..frames).flat_map(|frame| buffers.iter().map(move |buffer| buffer[frame])).collect()
}

#[cfg(test)]
mod tests {
    use super::interleave;

    #[test]
    fn keeps_single_buffer() {
        assert_eq!(interleave(&[&[0.1, 0.2, 0.3, 0.4]]), vec![0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn interleaves_planar_buffers() {
        assert_eq!(interleave(&[&[1.0, 2.0], &[-1.0, -2.0]]), vec![1.0, -1.0, 2.0, -2.0]);
    }

    #[test]
    fn empty_without_buffers() {
        assert_eq!(interleave(&[]), Vec::<f32>::new());
    }
}
