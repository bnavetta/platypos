use arr_macro::arr;
use log::info;
use spin::Once;
use x86_64::instructions::segmentation::set_cs;
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

use crate::config::MAX_PROCESSORS;
use crate::kernel_state;
use crate::topology::processor::local_id;

/// IST index for the stack processor exceptions should be handled on.
pub const FAULT_IST_INDEX: u16 = 0;

/// Size in page frames of the stack to allocate for an interrupt or fault using the IST feature
const INTERRUPT_STACK_FRAMES: usize = 2;

/// Allocate an `INTERRUPT_STACK_FRAMES`-sized stack. Returns a pointer to the top of the stack
fn allocate_interrupt_stack() -> VirtAddr {
    let fault_stack = kernel_state()
        .frame_allocator()
        .allocate_pages(INTERRUPT_STACK_FRAMES)
        .expect("Could not allocate interrupt stack");

    fault_stack.start_address() + INTERRUPT_STACK_FRAMES as u64 * 4096
}

/// Create a TSS for a newly-started processor
fn create_tss() -> TaskStateSegment {
    let fault_stack = allocate_interrupt_stack();

    let mut tss = TaskStateSegment::new();
    tss.interrupt_stack_table[FAULT_IST_INDEX as usize] = fault_stack;
    tss
}

// Unfortunately, arr! doesn't support `const` variables for array initialization. That means we
// have to pick a constant and hope it's big enough. There's an assertion in `install` which will
// at least fail fast if not.
// See https://github.com/JoshMcguigan/arr_macro/issues/2 for more context.
static TSS_PERPROCESSOR: [Once<TaskStateSegment>; 8] = arr![Once::new(); 8];

struct GdtAndSelectors {
    gdt: GlobalDescriptorTable,
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

// We could theoretically have one global GDT containing a TSS for each processor. However,
// the GDT only supports up to 8 entries, and since one is used for the code segment, that would
// limit support to 7 CPUs. It's more scalable to have a per-processor GDT, and it doesn't take
// up that much extra memory.
static GDT_SELECTORS_PERPROCESSOR: [Once<GdtAndSelectors>; 8] = arr![Once::new(); 8];

/// Create and install a TSS and GDT for the current processor. This must be called once on every
/// processor, since processors do not share a TSS or GDT.
pub fn install() {
    assert!(
        MAX_PROCESSORS <= 8,
        "GDT and TSS initialization code only supports up to 8 processors"
    );

    let id = local_id();

    let tss = TSS_PERPROCESSOR[id].call_once(|| create_tss());

    let gdt_and_selectors = GDT_SELECTORS_PERPROCESSOR[id].call_once(|| {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(tss));

        GdtAndSelectors {
            gdt,
            code_selector,
            tss_selector,
        }
    });

    gdt_and_selectors.gdt.load();

    unsafe {
        set_cs(gdt_and_selectors.code_selector);
        load_tss(gdt_and_selectors.tss_selector);
    }

    info!("Loaded GDT and selectors on processor {}", id);
}
