pub mod context;
mod multiprocessor;

use log::info;

pub fn init() {
    info!("Starting scheduler");
    multiprocessor::boot_application_processors();
}
