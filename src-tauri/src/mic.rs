use std::ffi::c_void;
use std::mem::size_of;
use std::ptr::null;

#[repr(C)]
struct PropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectGetPropertyDataSize(
        id: u32,
        address: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
    ) -> i32;
    fn AudioObjectGetPropertyData(
        id: u32,
        address: *const PropertyAddress,
        qualifier_size: u32,
        qualifier: *const c_void,
        size: *mut u32,
        data: *mut c_void,
    ) -> i32;
}

const SYSTEM_OBJECT: u32 = 1;
const SCOPE_GLOBAL: u32 = u32::from_be_bytes(*b"glob");
const PROCESS_OBJECT_LIST: u32 = u32::from_be_bytes(*b"prs#");
const PROCESS_PID: u32 = u32::from_be_bytes(*b"ppid");
const PROCESS_IS_RUNNING_INPUT: u32 = u32::from_be_bytes(*b"piri");

fn address(selector: u32) -> PropertyAddress {
    PropertyAddress { selector, scope: SCOPE_GLOBAL, element: 0 }
}

fn read<T: Copy + Default>(id: u32, selector: u32) -> Option<T> {
    let mut value = T::default();
    let mut size = size_of::<T>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(id, &address(selector), 0, null(), &mut size, &mut value as *mut T as *mut c_void)
    };
    (status == 0).then_some(value)
}

fn process_objects() -> Vec<u32> {
    let addr = address(PROCESS_OBJECT_LIST);
    let mut size = 0;
    if unsafe { AudioObjectGetPropertyDataSize(SYSTEM_OBJECT, &addr, 0, null(), &mut size) } != 0 {
        return Vec::new();
    }
    let mut ids = vec![0u32; size as usize / size_of::<u32>()];
    let status = unsafe {
        AudioObjectGetPropertyData(SYSTEM_OBJECT, &addr, 0, null(), &mut size, ids.as_mut_ptr() as *mut c_void)
    };
    if status != 0 {
        return Vec::new();
    }
    ids.truncate(size as usize / size_of::<u32>());
    ids
}

pub fn processes() -> Vec<(u32, i32)> {
    process_objects().into_iter().filter_map(|id| read::<i32>(id, PROCESS_PID).map(|pid| (id, pid))).collect()
}

pub fn input_pids() -> Vec<i32> {
    processes()
        .into_iter()
        .filter(|&(id, _)| read::<u32>(id, PROCESS_IS_RUNNING_INPUT).unwrap_or(0) != 0)
        .map(|(_, pid)| pid)
        .collect()
}
