use std::sync::Arc;

use lazy_static::lazy_static;
use parking_lot::RwLock;

use super::PackageManager;

lazy_static! {
    static ref PACKAGE_MANAGER: RwLock<Option<Arc<PackageManager>>> = RwLock::new(None);
}

pub fn initialize_package_manager(pm: &Arc<PackageManager>) {
    *PACKAGE_MANAGER.write() = Some(pm.clone());
}

pub fn finalize_package_manager() {
    *PACKAGE_MANAGER.write() = None;
}

pub fn package_manager_checked() -> anyhow::Result<Arc<PackageManager>> {
    PACKAGE_MANAGER
        .read()
        .as_ref()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Package manager is not initialized!"))
}

pub fn package_manager() -> Arc<PackageManager> {
    package_manager_checked().unwrap()
}
