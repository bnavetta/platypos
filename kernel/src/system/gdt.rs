use log::info;
use spin::Once;
use x86_64::instructions::segmentation::set_cs;
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::paging::{Mapper, Page, PageTableFlags};
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
        let kernel_state = crate::kernel_state();

        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[FAULT_IST_INDEX as usize] = VirtAddr::new(FAULT_STACK_END); // Use the end because the stack grows down

        let fault_stack = kernel_state
            .frame_allocator()
            .allocate_pages(FAULT_STACK_FRAMES as usize)
            .expect("Failed to allocate fault stack");

        info!(
            "Fault-handler stack starting at physical address {:#x}, mapped to {:#x}",
            fault_stack.start_phys_address().as_u64(),
            FAULT_STACK_START
        );

        kernel_state.with_page_table(|pt| {
            let first_frame = fault_stack.start_frame();
            let first_page = Page::from_start_address(VirtAddr::new(FAULT_STACK_START))
                .expect("Fault stack not page aligned");

            unsafe {
                let mut mapper = pt.active_4kib_mapper();
                let mut allocator = kernel_state.frame_allocator().page_table_allocator();

                for i in 0..FAULT_STACK_FRAMES {
                    mapper
                        .map_to(
                            first_page + i,
                            first_frame + i,
                            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                            &mut allocator,
                        )
                        .expect("Unable to map fault stack")
                        .flush();
                }
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
