use cpal::{Data, I24, SampleFormat, SizedSample, U24};

fn convert<T: SizedSample>(data: &Data) -> Vec<f32>
where
    f32: cpal::FromSample<T>,
{
    data.as_slice::<T>().map(|samples| samples.iter().map(|&s| s.to_sample::<f32>()).collect()).unwrap_or_default()
}

pub fn to_f32(data: &Data) -> Vec<f32> {
    match data.sample_format() {
        SampleFormat::I8 => convert::<i8>(data),
        SampleFormat::F32 => convert::<f32>(data),
        SampleFormat::I16 => convert::<i16>(data),
        SampleFormat::I24 => convert::<I24>(data),
        SampleFormat::I32 => convert::<i32>(data),
        SampleFormat::I64 => convert::<i64>(data),
        SampleFormat::U8 => convert::<u8>(data),
        SampleFormat::U16 => convert::<u16>(data),
        SampleFormat::U24 => convert::<U24>(data),
        SampleFormat::U32 => convert::<u32>(data),
        SampleFormat::U64 => convert::<u64>(data),
        SampleFormat::F64 => convert::<f64>(data),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data<T: SizedSample>(samples: &mut [T]) -> Data {
        unsafe { Data::from_parts(samples.as_mut_ptr().cast(), samples.len(), T::FORMAT) }
    }

    fn assert_close(actual: Vec<f32>, expected: &[f32]) {
        assert_eq!(actual.len(), expected.len());
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 1e-4, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn keeps_f32() {
        assert_eq!(to_f32(&data(&mut [0.5f32, -0.25])), vec![0.5, -0.25]);
    }

    #[test]
    fn converts_i16_to_f32() {
        assert_close(to_f32(&data(&mut [i16::MAX, 0, i16::MIN])), &[1.0, 0.0, -1.0]);
    }

    #[test]
    fn converts_u16_to_f32() {
        assert_close(to_f32(&data(&mut [u16::MAX, 32768, 0])), &[1.0, 0.0, -1.0]);
    }

    #[test]
    fn converts_24_bit_samples_to_f32() {
        assert_close(
            to_f32(&data(&mut [
                I24::new_unchecked((1 << 23) - 1),
                I24::new_unchecked(0),
                I24::new_unchecked(-(1 << 23)),
            ])),
            &[0.999_999, 0.0, -1.0],
        );
    }
}
