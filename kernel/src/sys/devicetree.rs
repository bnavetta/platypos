//! DeviceTree-based topology management

use core::alloc::Allocator;
use core::alloc::Layout;
use core::convert::TryInto;
use core::fmt;
use core::mem;
use core::mem::MaybeUninit;

use fdt_rs::prelude::*;
use fdt_rs::base::*;
use fdt_rs::index::*;

use crate::alloc::early;

pub struct DeviceTree {
    index: DevTreeIndex<'static, 'static>
}

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
            Ok(layout) => {
                // DevTreeIndex::get_layout returns a layout that isn't /quite/ big enough in practice, so add some padding
                let (padded, _) = layout.extend(Layout::new::<[u8; 64]>()).unwrap();
                padded
            },
            Err(err) => panic!("Invalid FDT at {:p}: {}", fdt_ptr, err)
        };
        let _ = writeln!(log, "Allocating {:?} for DeviceTree index", layout);

        let buf = early::ALLOC.allocate_zeroed(layout).expect("Could not early-alloc for DeviceTree index");
        // Round-trip through MaybeUninit rather than just using buf.as_mut() in order to correctly preserve Rust's slice layout
        // The allocator returns a NonNull<[u8]>, which is a raw slice pointer, not just a pointer to the memory.
        // Since we allocated zeroed memory, it's safe to call slice_assume_init_mut.
        let raw_slice = MaybeUninit::slice_assume_init_mut(buf.as_uninit_slice_mut());

        let index = DevTreeIndex::new(tree, raw_slice).expect("Could not build DeviceTree index");
        let _ = writeln!(log, "Built DeviceTree index");

        DeviceTree {
            index
        }
    }

    pub fn info<W: fmt::Write>(&self, mut w: W) -> fmt::Result {
        writeln!(w, "Device Tree:")?;
        for node in self.index.root().children() {
            node_info(&mut w, &node, 0)?;
        }

        Ok(())
    }
}

fn node_info<W: fmt::Write>(w: &mut W, node: &DevTreeIndexNode, indent: usize) -> fmt::Result {
    let node_name = match node.name() {
        Ok(s) => s,
        Err(_) => {
            writeln!(w, "Skipping malformed node...")?;
            return Ok(());
        }
    };

    for _ in 0..indent {
        write!(w, "  ")?;
    }

    if node_name.starts_with("memory@") {
        let parent = node.parent().expect("memory node cannot be the root");
        let address_cells = get_prop(&parent, "#address-cells")
            .expect("parent must contain an #address-cells property")
            .u32(0).expect("malformed #address-cells property");
        assert_eq!(address_cells, 2, "#address-cells should be 2 on 64-bit RISC-V");
        let size_cells = get_prop(&parent, "#size-cells")
            .expect("parent must contain a #size-cells property")
            .u32(0).expect("malformed #size-cells property");
        assert_eq!(size_cells, 2, "#size-cells should be 2 on 64-bit RISC-V");

        let reg_prop = get_prop(node, "reg")
            .expect("memory node must have a reg property");
        assert!(reg_prop.propbuf().len() % (2 * mem::size_of::<u64>()) == 0);
        let ranges = reg_prop.propbuf()
            .chunks_exact(2 * mem::size_of::<u64>())
            .map(|range| {
                let start = u64::from_be_bytes(range[..mem::size_of::<u64>()].try_into().unwrap());
                let length = u64::from_be_bytes(range[mem::size_of::<u64>()..].try_into().unwrap());
                (start, length)
            });

        writeln!(w, "- Memory @ {}", node_name)?;

        for (start, length) in ranges {
            for _ in 0..indent {
                write!(w, "  ")?;
            }
            writeln!(w, "    * {:#0x} - {:#0x} ({} bytes)", start, start + length, length)?;
        }
    } else {
        write!(w, "- {}", node_name)?;

        if let Some(compatible) = get_prop(node, "compatible") {
            write!(w, " (compatible: {})", compatible.str().expect("Malformed `compatible` property"))?;
        }

        writeln!(w)?;

        for child in node.children() {
            node_info(w, &child, indent + 1)?;
        }
    }

    Ok(())
}

/// Splits a DeviceTree name into its node name and unit address components. DeviceTree nodes
/// are named with the format `node-name@unit-address`.
/// 
/// Returns `None` if the name does not match the expected format.
fn split_name(name: &str) -> Option<(&str, &str)> {
    name.split_once("@")
}

fn get_prop<'p, 'a, 'i, 'dt>(node: &'p DevTreeIndexNode<'a, 'i, 'dt>, name: &str) -> Option<DevTreeIndexProp<'a, 'i, 'dt>> {
    node.props().find(|p| p.name() == Ok(name))
}

fn read_u64_array<'a, 'i, 'dt>(prop: DevTreeIndexProp<'a, 'i, 'dt>) -> impl Iterator<Item=u64> + 'dt {
    assert!(prop.propbuf().len() % mem::size_of::<u64>() == 0);
    prop.propbuf()
        .chunks_exact(mem::size_of::<u64>())
        .map(|elem| {
            let bytes = elem.try_into().unwrap();
            u64::from_be_bytes(bytes)
        })
}