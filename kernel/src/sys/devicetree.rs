//! DeviceTree-based topology management

use core::alloc::Allocator;
use core::fmt;

use fdt_rs::base::*;
use fdt_rs::index::*;

use crate::alloc::early;

pub struct DeviceTree {
    index: DevTreeIndex<'static, 'static>
}

static mut TEST_BUF: [u8; 8192] = [0u8; 8192];

impl DeviceTree {
    /// Initializes the DeviceTree system given a pointer to the FDT in memory.
    pub unsafe fn new<W: fmt::Write>(fdt_ptr: *const u8, mut log: W) -> DeviceTree {
        // TODO: real logging
        let _ = writeln!(log, "Reading FDT from {:p}", fdt_ptr);

        let tree: DevTree<'static> = match DevTree::from_raw_pointer(fdt_ptr) {
            Ok(tree) => tree,
            Err(err) => panic!("Invalid FDT pointer {:p}: {}", fdt_ptr, err)
        };
        let _ = writeln!(log, "DeviceTree version {}", tree.version());

        let layout = match DevTreeIndex::get_layout(&tree) {
            Ok(layout) => layout,
            Err(err) => panic!("Invalid FDT at {:p}: {}", fdt_ptr, err)
        };
        let _ = writeln!(log, "Allocating {:?} for DeviceTree index", layout);

        // let mut buf = early::ALLOC.allocate_zeroed(layout).expect("Could not early-alloc for DeviceTree index");
        let buf = TEST_BUF.as_mut();
        // let raw_slice: &'static mut [u8] = buf.as_mut();
        let _ = writeln!(log, "Allocated DeviceTree index at {:p}", buf);
        let index = DevTreeIndex::new(tree, buf).expect("Could not build DeviceTree index");
        let _ = writeln!(log, "Built DeviceTree index");

        DeviceTree {
            index
        }
    }

    pub fn info<W: fmt::Write>(&self, mut w: W) -> fmt::Result {
        writeln!(w, "Memory Map:")?;
        for node in self.index.root().children() {
            let name = match node.name() {
                Ok(s) if s.starts_with("memory") => s,
                _ => continue
            };

            writeln!(w, "- {}", name)?;
        }

        Ok(())
    }
}