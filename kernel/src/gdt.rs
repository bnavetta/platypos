use bootloader::BootInfo;
use log::info;
use spin::Once;
use x86_64::instructions::segmentation::set_cs;
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::paging::{Page, PhysFrame, Size4KiB};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

struct GdtAndSelectors {
    gdt: GlobalDescriptorTable,
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

static TSS: Once<TaskStateSegment> = Once::new();
static GDT: Once<GdtAndSelectors> = Once::new();

// 8 KiB should be plenty
// We use a separate virtual address mapping instead of the one returned from the frame allocator
// so that we can make sure there's an unmapped guard page after the stack.
const FAULT_STACK_FRAMES: u64 = 2;
const FAULT_STACK_START: u64 = 0xfffffbffffffc000; // should be 4 pages below physical memory map
const FAULT_STACK_END: u64 = FAULT_STACK_START + FAULT_STACK_FRAMES * 4096;
pub const FAULT_IST_INDEX: u16 = 0;

pub fn init() {
    let tss = TSS.call_once(|| {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[FAULT_IST_INDEX as usize] = VirtAddr::new(FAULT_STACK_END); // Use the end because the stack grows down

        let fault_stack = crate::memory::frame::allocate_frames(FAULT_STACK_FRAMES as usize)
            .expect("Failed to allocate fault stack");

        crate::memory::page_table::with_page_table(|pt| {
            let first_frame = PhysFrame::from_start_address(
                pt.translate(VirtAddr::from_ptr(fault_stack))
                    .expect("Could not translate fault stack"),
            )
            .expect("Fault stack not page aligned");
            let first_page: Page<Size4KiB> =
                Page::from_start_address(VirtAddr::new(FAULT_STACK_START)).expect("Fault stack not page aligned");

            for i in 0..FAULT_STACK_FRAMES {
                unsafe { pt.map_page(first_frame + i, first_page + i, true) }
                    .expect("Unable to map fault stack");
            }
        });

        tss
    });

    let gdt_and_selectors = GDT.call_once(|| {
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

    info!("Loaded GDT and selectors");
}