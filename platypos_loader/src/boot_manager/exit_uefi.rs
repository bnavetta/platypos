use core::fmt::Write;
use core::mem;

use log::{debug, info};
use uart_16550::SerialPort;
use uefi::prelude::*;
use x86_64::VirtAddr;

use super::handoff::Handoff;
use super::{BootManager, Stage};
use crate::util::halt_loop;

pub struct ExitUefiBootServices {
    /// Address of the kernel entry point
    pub kernel_entry_addr: VirtAddr,
}

impl Stage for ExitUefiBootServices {
    type SystemTableView = Boot;
}

impl BootManager<ExitUefiBootServices> {
    pub fn exit_boot_services(self) -> BootManager<Handoff> {
        // Add some padding in case the memory map changes size
        let memory_map_size = self.system_table.boot_services().memory_map_size() + 256;
        debug!("Allocating {} bytes for final memory map", memory_map_size);
        let mut memory_map_buffer = vec![0u8; memory_map_size];

        info!("Exiting UEFI boot services");

        let mut debug_port = unsafe { SerialPort::new(0x3F8) };
        debug_port.init();

        let table = match self
            .system_table
            .exit_boot_services(self.image_handle, &mut memory_map_buffer)
        {
            Ok(comp) => {
                let (status, (table, _)) = comp.split();
                if status.is_success() {
                    table
                } else {
                    writeln!(
                        &mut debug_port,
                        "Warning exiting boot services: {:?}",
                        status
                    ).unwrap();
                    halt_loop();
                }
            }
            Err(err) => {
                writeln!(&mut debug_port, "Error exiting boot services: {:?}", err).unwrap();
                halt_loop();
            }
        };

        writeln!(&mut debug_port, "Exited UEFI boot services").unwrap();

        // Can't deallocate it since we no longer have an allocator
        mem::forget(memory_map_buffer);

        // TODO: create memory map to hand to kernel

        BootManager {
            stage: Handoff {
                kernel_entry_addr: self.stage.kernel_entry_addr,
                debug_port,
            },
            system_table: table,
            image_handle: self.image_handle,
            page_table: self.page_table,
            page_table_address: self.page_table_address,
        }
    }
}
