use core_foundation::array::CFArray;
use core_foundation::base::{CFType, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::string::{CFString, CFStringRef};
use std::ptr::null;

use crate::tab_title;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    static kAXTrustedCheckOptionPrompt: CFStringRef;
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    fn AXUIElementCreateApplication(pid: i32) -> CFTypeRef;
    fn AXUIElementCopyAttributeValue(element: CFTypeRef, attribute: CFStringRef, value: *mut CFTypeRef) -> i32;
}

const MAX_DEPTH: u8 = 15;

pub fn is_trusted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

pub fn request_trust() {
    let key = unsafe { CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt) };
    let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
    unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) };
}

fn attribute(element: &CFType, name: &str) -> Option<CFType> {
    let mut value: CFTypeRef = null();
    let name = CFString::new(name);
    let status = unsafe { AXUIElementCopyAttributeValue(element.as_CFTypeRef(), name.as_concrete_TypeRef(), &mut value) };
    (status == 0 && !value.is_null()).then(|| unsafe { CFType::wrap_under_create_rule(value) })
}

fn text(element: &CFType, name: &str) -> Option<String> {
    attribute(element, name)?.downcast_into::<CFString>().map(|s| s.to_string())
}

fn children(element: &CFType) -> Vec<CFType> {
    attribute(element, "AXChildren")
        .and_then(|value| value.downcast_into::<CFArray>())
        .map(|array| array.iter().map(|child| unsafe { CFType::wrap_under_get_rule(*child) }).collect())
        .unwrap_or_default()
}

fn collect_tabs(element: &CFType, depth: u8, tabs: &mut Vec<String>) {
    if depth == 0 || text(element, "AXRole").as_deref() == Some("AXWebArea") {
        return;
    }
    tabs.extend(["AXTitle", "AXDescription"].iter().filter_map(|name| text(element, name)).filter_map(|label| tab_title::from_label(&label)));
    for child in children(element) {
        collect_tabs(&child, depth - 1, tabs);
    }
}

pub fn mic_tabs(pid: i32) -> Vec<String> {
    if !is_trusted() {
        return Vec::new();
    }
    let app = unsafe { CFType::wrap_under_create_rule(AXUIElementCreateApplication(pid)) };
    let mut tabs = Vec::new();
    collect_tabs(&app, MAX_DEPTH, &mut tabs);
    tabs
}
