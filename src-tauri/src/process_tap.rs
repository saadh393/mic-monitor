use block2::RcBlock;
use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::string::CFString;
use objc2::AnyThread;
use objc2_core_audio::{
    kAudioAggregateDeviceIsPrivateKey, kAudioAggregateDeviceNameKey, kAudioAggregateDeviceTapAutoStartKey, kAudioAggregateDeviceTapListKey, kAudioAggregateDeviceUIDKey, kAudioObjectPropertyElementMain,
    kAudioObjectPropertyScopeGlobal, kAudioSubTapUIDKey, kAudioTapPropertyFormat, AudioDeviceCreateIOProcIDWithBlock, AudioDeviceDestroyIOProcID, AudioDeviceIOProcID, AudioDeviceStart, AudioDeviceStop,
    AudioHardwareCreateAggregateDevice, AudioHardwareCreateProcessTap, AudioHardwareDestroyAggregateDevice, AudioHardwareDestroyProcessTap, AudioObjectGetPropertyData, AudioObjectPropertyAddress, CATapDescription,
    CATapMuteBehavior,
};
use objc2_core_audio_types::{AudioBufferList, AudioStreamBasicDescription, AudioTimeStamp};
use objc2_foundation::{NSArray, NSNumber};
use std::ffi::{c_void, CStr};
use std::mem::size_of;
use std::ptr::{null, NonNull};
use std::sync::Mutex;

use crate::interleave::interleave;

type IoBlock = RcBlock<dyn Fn(NonNull<AudioTimeStamp>, NonNull<AudioBufferList>, NonNull<AudioTimeStamp>, NonNull<AudioBufferList>, NonNull<AudioTimeStamp>)>;

pub struct ProcessTap {
    pub rate: u32,
    pub channels: u16,
    tap: u32,
    device: u32,
    proc_id: AudioDeviceIOProcID,
    _block: IoBlock,
}

unsafe impl Send for ProcessTap {}

impl ProcessTap {
    pub fn stop(self) {
        unsafe {
            AudioDeviceStop(self.device, self.proc_id);
            AudioDeviceDestroyIOProcID(self.device, self.proc_id);
            AudioHardwareDestroyAggregateDevice(self.device);
            AudioHardwareDestroyProcessTap(self.tap);
        }
    }
}

fn ok(status: i32, what: &str) -> Result<(), String> {
    if status == 0 {
        Ok(())
    } else {
        Err(format!("{what} failed with status {status}"))
    }
}

fn key(name: &CStr) -> CFString {
    CFString::new(&name.to_string_lossy())
}

fn dictionary(pairs: &[(&CStr, CFType)]) -> CFDictionary<CFString, CFType> {
    let pairs: Vec<(CFString, CFType)> = pairs.iter().map(|(name, value)| (key(name), value.clone())).collect();
    CFDictionary::from_CFType_pairs(&pairs)
}

fn create_tap(processes: &[u32]) -> Result<(u32, String), String> {
    let ids: Vec<_> = processes.iter().map(|&id| NSNumber::new_u32(id)).collect();
    let description = unsafe { CATapDescription::initStereoMixdownOfProcesses(CATapDescription::alloc(), &NSArray::from_retained_slice(&ids)) };
    unsafe {
        description.setPrivate(true);
        description.setMuteBehavior(CATapMuteBehavior::Unmuted);
    }
    let mut tap = 0;
    ok(unsafe { AudioHardwareCreateProcessTap(Some(&description), &mut tap) }, "AudioHardwareCreateProcessTap")?;
    Ok((tap, unsafe { description.UUID() }.UUIDString().to_string()))
}

fn format(tap: u32) -> Result<AudioStreamBasicDescription, String> {
    let address = AudioObjectPropertyAddress { mSelector: kAudioTapPropertyFormat, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain };
    let mut format = AudioStreamBasicDescription { mSampleRate: 0.0, mFormatID: 0, mFormatFlags: 0, mBytesPerPacket: 0, mFramesPerPacket: 0, mBytesPerFrame: 0, mChannelsPerFrame: 0, mBitsPerChannel: 0, mReserved: 0 };
    let mut size = size_of::<AudioStreamBasicDescription>() as u32;
    let status = unsafe { AudioObjectGetPropertyData(tap, NonNull::from(&address), 0, null(), NonNull::from(&mut size), NonNull::from(&mut format).cast::<c_void>()) };
    ok(status, "reading the tap format")?;
    Ok(format)
}

fn create_device(uuid: &str) -> Result<u32, String> {
    let sub_tap = dictionary(&[(kAudioSubTapUIDKey, CFString::new(uuid).as_CFType())]);
    let description = dictionary(&[
        (kAudioAggregateDeviceUIDKey, CFString::new(&format!("com.sadh.micmonitor.tap.{uuid}")).as_CFType()),
        (kAudioAggregateDeviceNameKey, CFString::new("Mic Monitor Meeting Audio").as_CFType()),
        (kAudioAggregateDeviceIsPrivateKey, CFBoolean::true_value().as_CFType()),
        (kAudioAggregateDeviceTapAutoStartKey, CFBoolean::true_value().as_CFType()),
        (kAudioAggregateDeviceTapListKey, CFArray::from_CFTypes(&[sub_tap]).as_CFType()),
    ]);
    let mut device = 0;
    let description = unsafe { &*(description.as_concrete_TypeRef() as *const objc2_core_foundation::CFDictionary) };
    ok(unsafe { AudioHardwareCreateAggregateDevice(description, NonNull::from(&mut device)) }, "AudioHardwareCreateAggregateDevice")?;
    Ok(device)
}

unsafe fn buffers<'a>(list: NonNull<AudioBufferList>) -> Vec<&'a [f32]> {
    let list = list.as_ref();
    let first = list.mBuffers.as_ptr();
    (0..list.mNumberBuffers as usize)
        .map(|i| {
            let buffer = &*first.add(i);
            if buffer.mData.is_null() {
                &[][..]
            } else {
                std::slice::from_raw_parts(buffer.mData as *const f32, buffer.mDataByteSize as usize / size_of::<f32>())
            }
        })
        .collect()
}

fn start_device(device: u32, on_samples: impl FnMut(&[f32]) + Send + 'static) -> Result<(AudioDeviceIOProcID, IoBlock), String> {
    let on_samples = Mutex::new(on_samples);
    let block: IoBlock = RcBlock::new(move |_: NonNull<AudioTimeStamp>, input: NonNull<AudioBufferList>, _: NonNull<AudioTimeStamp>, _: NonNull<AudioBufferList>, _: NonNull<AudioTimeStamp>| {
        let samples = interleave(&unsafe { buffers(input) });
        if let Ok(mut on_samples) = on_samples.lock() {
            on_samples(&samples);
        }
    });
    let mut proc_id: AudioDeviceIOProcID = None;
    ok(unsafe { AudioDeviceCreateIOProcIDWithBlock(NonNull::from(&mut proc_id), device, None, RcBlock::as_ptr(&block)) }, "AudioDeviceCreateIOProcIDWithBlock")?;
    if let Err(e) = ok(unsafe { AudioDeviceStart(device, proc_id) }, "AudioDeviceStart") {
        unsafe { AudioDeviceDestroyIOProcID(device, proc_id) };
        return Err(e);
    }
    Ok((proc_id, block))
}

pub fn start(processes: Vec<u32>, on_samples: impl FnMut(&[f32]) + Send + 'static) -> Result<ProcessTap, String> {
    if processes.is_empty() {
        return Err("the meeting app has no Core Audio process to tap".into());
    }
    let (tap, uuid) = create_tap(&processes)?;
    let (format, device) = match format(tap).and_then(|format| Ok((format, create_device(&uuid)?))) {
        Ok(found) => found,
        Err(e) => {
            unsafe { AudioHardwareDestroyProcessTap(tap) };
            return Err(e);
        }
    };
    match start_device(device, on_samples) {
        Ok((proc_id, block)) => Ok(ProcessTap { rate: format.mSampleRate as u32, channels: format.mChannelsPerFrame as u16, tap, device, proc_id, _block: block }),
        Err(e) => {
            unsafe {
                AudioHardwareDestroyAggregateDevice(device);
                AudioHardwareDestroyProcessTap(tap);
            }
            Err(e)
        }
    }
}
