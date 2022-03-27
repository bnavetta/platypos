use x86_64::VirtAddr;

#[derive(Debug)]
pub struct Frame {
    base_pointer: VirtAddr,
    pub instruction_pointer: VirtAddr,
    stack_pointer: VirtAddr,
}

impl Frame {
    #[inline(always)]
    pub fn current() -> Frame {
        let ip: u64;
        let sp: u64;
        let bp: u64;
        // Safety: this only reads registers
        unsafe {
            asm!(
                "lea {ip}, [rip]",
                "mov {sp}, rsp",
                "mov {bp}, rbp",
                ip = out(reg) ip,
                sp = out(reg) sp,
                bp = out(reg) bp,
                options(nostack, nomem)
            );
        }

        Frame {
            base_pointer: VirtAddr::new(bp),
            stack_pointer: VirtAddr::new(sp),
            instruction_pointer: VirtAddr::new(ip),
        }
    }

    /// Attempt to retrieve the stack frame of this frame's caller.
    ///
    /// # Unsafety
    /// This reads from memory to hopefully walk up the stack. It's _very_ unsafe.
    pub unsafe fn parent(&self) -> Option<Frame> {
        // For more on x86_64 backtraces, see https://github.com/gz/backtracer and https://wiki.osdev.org/Stack_Trace

        // This assumes we start with a null RBP when the kernel begins to execute
        if self.base_pointer.is_null() {
            return None;
        }

        // The caller's return address should be right above the current stack frame, since it's pushed by the call instruction
        let parent_ip: u64 = self.base_pointer.as_ptr::<u64>().add(1).read();
        // The base pointer is just the value of the stack pointer at the start of the function
        let parent_sp = self.base_pointer;
        // And the parent's base pointer is pushed to the start of our stack frame
        let parent_bp = self.base_pointer.as_ptr::<u64>().read();

        Some(Frame {
            base_pointer: VirtAddr::new(parent_bp),
            instruction_pointer: VirtAddr::new(parent_ip),
            stack_pointer: parent_sp,
        })
    }
}
