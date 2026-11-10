use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::mpsc::{self, Sender};
use std::thread;

use crate::mic_samples;

pub struct MicCapture {
    pub rate: u32,
    pub channels: u16,
    stopper: Sender<()>,
}

impl MicCapture {
    pub fn stop(self) {
        let _ = self.stopper.send(());
    }
}

fn open(mut on_samples: impl FnMut(&[f32]) + Send + 'static) -> Result<(cpal::Stream, u32, u16), String> {
    let host = {
        #[cfg(target_os = "macos")]
        {
            cpal::host_from_id(cpal::HostId::CoreAudio).map_err(|e| e.to_string())?
        }
        #[cfg(not(target_os = "macos"))]
        {
            cpal::default_host()
        }
    };
    let device = host.default_input_device().ok_or("no input device")?;
    let supported = device.default_input_config().map_err(|e| e.to_string())?;
    let config = supported.config();
    let (rate, channels) = (config.sample_rate, config.channels);
    let device_name = device.description().map(|d| d.name().to_string()).unwrap_or_else(|_| "default input".into());
    let stream = device
        .build_input_stream_raw(
            config,
            supported.sample_format(),
            move |data, _| {
                let samples = mic_samples::to_f32(data);
                if !samples.is_empty() {
                    on_samples(&samples);
                }
            },
            move |e| eprintln!("microphone stream error on {device_name}: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, rate, channels))
}

pub fn start(on_samples: impl FnMut(&[f32]) + Send + 'static) -> Result<MicCapture, String> {
    let (ready, opened) = mpsc::channel();
    let (stopper, stopped) = mpsc::channel::<()>();
    thread::spawn(move || match open(on_samples) {
        Ok((stream, rate, channels)) => {
            let _ = ready.send(Ok((rate, channels)));
            let _ = stopped.recv();
            drop(stream);
        }
        Err(e) => {
            let _ = ready.send(Err(e));
        }
    });
    let (rate, channels) = opened.recv().map_err(|e| e.to_string())??;
    Ok(MicCapture { rate, channels, stopper })
}
