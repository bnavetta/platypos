//! DeviceTree-based topology management

use core::alloc::Allocator;
use core::alloc::Layout;
use core::convert::TryInto;
use core::fmt;
use core::mem;
use core::mem::MaybeUninit;

use fdt_rs::base::*;
use fdt_rs::index::*;
use fdt_rs::prelude::*;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::alloc::early;
use crate::driver::uart::UartConfig;

pub struct DeviceTree {
    index: DevTreeIndex<'static, 'static>,
}

impl DeviceTree {
    /// Initializes the DeviceTree system given a pointer to the FDT in memory.
    pub unsafe fn new(fdt_ptr: *const u8) -> DeviceTree {
        debug!("Reading FDT from {:p}", fdt_ptr);

        let tree: DevTree<'static> = match DevTree::from_raw_pointer(fdt_ptr) {
            Ok(tree) => tree,
            Err(err) => panic!("Invalid FDT pointer {:p}: {}", fdt_ptr, err),
        };

        info!(version = tree.version(), "Parsed FDT");

        let layout = match DevTreeIndex::get_layout(&tree) {
            Ok(layout) => {
                // DevTreeIndex::get_layout returns a layout that isn't /quite/ big enough in practice, so add some padding
                let (padded, _) = layout.extend(Layout::new::<[u8; 64]>()).unwrap();
                padded
            }
            Err(err) => panic!("Invalid FDT at {:p}: {}", fdt_ptr, err),
        };
        debug!("Allocating {} bytes for DeviceTree index", layout.size());

        let buf = early::ALLOC
            .allocate_zeroed(layout)
            .expect("Could not early-alloc for DeviceTree index");
        // Round-trip through MaybeUninit rather than just using buf.as_mut() in order to correctly preserve Rust's slice layout
        // The allocator returns a NonNull<[u8]>, which is a raw slice pointer, not just a pointer to the memory.
        // Since we allocated zeroed memory, it's safe to call slice_assume_init_mut.
        let raw_slice = MaybeUninit::slice_assume_init_mut(buf.as_uninit_slice_mut());

        let index = DevTreeIndex::new(tree, raw_slice).expect("Could not build DeviceTree index");
        info!("Built DeviceTree index");

        DeviceTree { index }
    }

    pub fn info(&self) {
        info!("Device Tree:");
        for node in self.index.root().children() {
            node_info(&node, Indent(0));
        }
    }

    /// Finds the first UART serial port in the device tree and returns its configuration.
    pub fn find_serial_port(&self) -> Option<UartConfig> {
        for node in self.index.compatible_nodes("ns16550a") {
            let (address_cells, size_cells) = get_addressing_cells(&node.parent().unwrap());
            let clock_frequency = get_prop(&node, "clock-frequency")
                .expect("clock-frequency property is required")
                .u32(0)
                .unwrap();
            let current_speed = get_prop(&node, "current-speed")
                .and_then(|p| p.u32(0).ok())
                .unwrap_or(0);
            let (register_start, register_size) = read_range_array(
                get_prop(&node, "reg")
                    .expect("reg property is required")
                    .propbuf(),
                address_cells,
                size_cells,
            )
            .unwrap()
            .next()
            .unwrap();
            // TODO: can there ever be multiple registers?
            let register_shift = get_prop(&node, "reg-shift")
                .and_then(|p| p.u32(0).ok())
                .unwrap_or(0);

            let name = node.name().unwrap_or("<unknown>");

            info!("Found serial port {}\n  Clock frequency: {} Hz\n  Current speed: {} BPS\n  MMIO registers: {:#0x}-{:#0x}\n  Register shift: {}", name, clock_frequency, current_speed, register_start, register_start + register_size, register_shift);

            return Some(UartConfig {
                base_address: register_start,
                clock_frequency,
            })
        }

        None
    }
}

fn node_info(node: &DevTreeIndexNode, indent: Indent) {
    let node_name = match node.name() {
        Ok(s) => s,
        Err(_) => {
            warn!("Skipping malformed node...");
            return;
        }
    };

    if node_name.starts_with("memory@") {
        let parent = node.parent().expect("memory node cannot be the root");
        let (address_cells, size_cells) = get_addressing_cells(&parent);
        assert_eq!(
            address_cells, 2,
            "#address-cells should be 2 on 64-bit RISC-V"
        );
        assert_eq!(size_cells, 2, "#size-cells should be 2 on 64-bit RISC-V");

        let reg_prop = get_prop(node, "reg").expect("memory node must have a reg property");

        let ranges = read_range_array(reg_prop.propbuf(), address_cells, size_cells).unwrap();

        info!("{}- Memory @ {}", indent, node_name);

        for (start, length) in ranges {
            info!(
                "{}    * {:#0x} - {:#0x} ({} bytes)",
                indent,
                start,
                start + length,
                length
            );
        }
    } else {
        let compatible = match get_prop(node, "compatible") {
            Some(compatible) => compatible.str().expect("Malformed `compatible` property"),
            None => "unknown",
        };
        info!("{}- {} (compatible: {})", indent, node_name, compatible);

        for child in node.children() {
            node_info(&child, indent.bump());
        }
    }
}

fn get_prop<'a, 'i, 'dt>(
    node: &DevTreeIndexNode<'a, 'i, 'dt>,
    name: &str,
) -> Option<DevTreeIndexProp<'a, 'i, 'dt>> {
    node.props().find(|p| p.name() == Ok(name))
}

/// Get the addressing information for children of `node`, based on its `#address-cells` and `#size-cells` properties.
fn get_addressing_cells(node: &DevTreeIndexNode<'_, '_, '_>) -> (u32, u32) {
    // Per section 2.3.5 of the DeviceTree spec, #address-cells defaults to 2 and #size-cells defaults to 1
    let address_cells = get_prop(node, "#address-cells")
        .and_then(|prop| prop.u32(0).ok())
        .unwrap_or(2);
    let size_cells = get_prop(node, "#size-cells")
        .and_then(|prop| prop.u32(0).ok())
        .unwrap_or(1);
    (address_cells, size_cells)
}

/// Decodes an array of `(address, length)` pairs. This format is commonly used in DeviceTree properties, such as `reg` and
/// `ranges`.
///
/// # Parameters
/// * `address_cells` - the number of `u32` cells making up each address
/// * `size_cells` - the number of `u32` cells making up each length
///
///
fn read_range_array(
    buf: &[u8],
    address_cells: u32,
    size_cells: u32,
) -> Option<impl Iterator<Item = (usize, usize)> + '_> {
    // TODO: return a result instead
    let pair_len = (address_cells + size_cells) as usize * mem::size_of::<u32>();
    let split_point = address_cells as usize * mem::size_of::<u32>();
    let read_address = cell_reader(address_cells);
    let read_size = cell_reader(size_cells);
    if buf.len() % pair_len != 0 {
        None
    } else {
        let iter = buf.chunks_exact(pair_len).map(move |pair| {
            let address = &pair[..split_point];
            let size = &pair[split_point..];
            (read_address(address), read_size(size))
        });
        Some(iter)
    }
}

fn cell_reader(cells: u32) -> fn(&[u8]) -> usize {
    match cells {
        1 => read_be_u32,
        2 => read_be_u64,
        _ => panic!("{}-cell integers not supported", cells),
    }
}

/// Helper to read a 32-bit big-endian integer, so that it can be 'static
fn read_be_u32(bytes: &[u8]) -> usize {
    u32::from_be_bytes(bytes.try_into().unwrap()) as usize
}

fn read_be_u64(bytes: &[u8]) -> usize {
    u64::from_be_bytes(bytes.try_into().unwrap()) as usize
}

// TODO: move to a printing/utils package
#[derive(Clone, Copy)]
struct Indent(usize);

impl Indent {
    fn bump(self) -> Indent {
        Indent(self.0 + 1)
    }
}

impl fmt::Display for Indent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for _ in 0..self.0 {
            f.write_str("  ")?;
        }
        Ok(())
    }
}
