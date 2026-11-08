use objc2_service_management::{SMAppService, SMAppServiceStatus};

pub fn register() {
    unsafe {
        let service = SMAppService::mainAppService();
        if service.status() != SMAppServiceStatus::NotRegistered {
            return;
        }
        if let Err(error) = service.registerAndReturnError() {
            eprintln!("login item registration failed: {error:?}");
        }
    }
}
