use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::AnyObject;
use objc2::AnyThread;
use objc2_avf_audio::{AVAudioFile, AVAudioPCMBuffer, AVFormatIDKey, AVNumberOfChannelsKey, AVSampleRateKey};
use objc2_core_audio_types::kAudioFormatMPEG4AAC;
use objc2_foundation::{NSDictionary, NSError, NSNumber, NSString, NSURL};
use std::path::Path;

use crate::audio_mix::RATE;

pub struct M4aWriter {
    file: Retained<AVAudioFile>,
    frames: u64,
}

unsafe impl Send for M4aWriter {}

fn message(error: Retained<NSError>) -> String {
    error.localizedDescription().to_string()
}

pub fn open(path: &Path) -> Result<M4aWriter, String> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    let keys = unsafe { [AVFormatIDKey, AVSampleRateKey, AVNumberOfChannelsKey] }.map(|key| key.expect("AVAudioSettings keys unavailable"));
    let values = [NSNumber::new_u32(kAudioFormatMPEG4AAC), NSNumber::new_f64(RATE as f64), NSNumber::new_u32(1)];
    let objects: Vec<&AnyObject> = values.iter().map(|value| value.as_ref()).collect();
    let settings = NSDictionary::<NSString, AnyObject>::from_slices(&keys, &objects);
    let file = autoreleasepool(|_| unsafe { AVAudioFile::initForWriting_settings_error(AVAudioFile::alloc(), &url, &settings) }).map_err(message)?;
    Ok(M4aWriter { file, frames: 0 })
}

impl M4aWriter {
    pub fn write(&mut self, samples: &[f32]) -> Result<(), String> {
        if samples.is_empty() {
            return Ok(());
        }
        autoreleasepool(|_| {
            let format = unsafe { self.file.processingFormat() };
            let buffer = unsafe { AVAudioPCMBuffer::initWithPCMFormat_frameCapacity(AVAudioPCMBuffer::alloc(), &format, samples.len() as u32) }.ok_or("could not allocate an audio buffer")?;
            unsafe {
                std::ptr::copy_nonoverlapping(samples.as_ptr(), (*buffer.floatChannelData()).as_ptr(), samples.len());
                buffer.setFrameLength(samples.len() as u32);
                self.file.writeFromBuffer_error(&buffer)
            }
            .map_err(message)
        })?;
        self.frames += samples.len() as u64;
        Ok(())
    }

    pub fn finish(self) -> Result<u64, String> {
        let frames = self.frames;
        autoreleasepool(|_| drop(self.file));
        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_playable_aac() {
        let path = std::env::temp_dir().join(format!("mic-monitor-test-{}.m4a", std::process::id()));
        let tone: Vec<f32> = (0..48_000).map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.5).collect();
        let mut writer = open(&path).unwrap();
        writer.write(&tone[..24_000]).unwrap();
        writer.write(&tone[24_000..]).unwrap();
        assert_eq!(writer.finish().unwrap(), 48_000);
        let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
        let (format_id, rate, channels, length) = autoreleasepool(|_| {
            let file = unsafe { AVAudioFile::initForReading_error(AVAudioFile::alloc(), &url) }.unwrap();
            let format = unsafe { file.fileFormat() };
            let description = unsafe { format.streamDescription().as_ref() };
            (description.mFormatID, description.mSampleRate, description.mChannelsPerFrame, unsafe { file.length() })
        });
        std::fs::remove_file(&path).unwrap();
        assert_eq!(format_id, kAudioFormatMPEG4AAC);
        assert_eq!(rate, 48_000.0);
        assert_eq!(channels, 1);
        assert!((length - 48_000).abs() <= 2_112, "length {length}");
    }
}
