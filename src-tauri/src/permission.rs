use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};

pub fn status() -> &'static str {
    let media = unsafe { AVMediaTypeAudio }.expect("AVMediaTypeAudio unavailable");
    match unsafe { AVCaptureDevice::authorizationStatusForMediaType(media) } {
        AVAuthorizationStatus::Authorized => "granted",
        AVAuthorizationStatus::NotDetermined => "undetermined",
        _ => "denied",
    }
}

pub fn request(on_done: impl Fn() + 'static) {
    let media = unsafe { AVMediaTypeAudio }.expect("AVMediaTypeAudio unavailable");
    let block = RcBlock::new(move |_: Bool| on_done());
    unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(media, &block) };
}
