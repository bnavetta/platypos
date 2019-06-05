use core::mem;

use x86_64::{PhysAddr, VirtAddr};

/// x86-64 context structure
#[derive(Debug)]
#[repr(C)]
pub struct Context {
    /// Address of the top-level page directory
    page_directory: usize,

    // FLAGS register
    flags: usize,

    /// Stack pointer
    rsp: usize,

    // These are the callee-save registers for the System V AMD64 ABI
    // See https://en.wikipedia.org/wiki/X86_calling_conventions#System_V_AMD64_ABI
    rbx: usize,
    rbp: usize,
    r12: usize,
    pub r13: usize,
    pub r14: usize,
    r15: usize,
}

impl Context {
    pub fn new(page_directory: PhysAddr, stack_pointer: VirtAddr) -> Context {
        Context {
            page_directory: page_directory.as_u64() as usize,
            flags: 0,
            rsp: stack_pointer.as_u64() as usize,
            rbp: 0,
            rbx: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        }
    }

    pub fn calling(
        page_directory: PhysAddr,
        stack_pointer: VirtAddr,
        function: fn(usize, usize, usize, usize) -> !,
        arg0: usize,
        arg1: usize,
        arg2: usize,
        arg3: usize,
    ) -> Context {
        let mut context = Context::new(page_directory, stack_pointer);

        // See note in context_initial_func on the calling convention
        context.rbx = function as usize;
        context.r12 = arg0;
        context.r13 = arg1;
        context.r14 = arg2;
        context.r15 = arg3;

        unsafe {
            context.push_stack(context_initial_func as usize);
        }

        context
    }

    /// Push a value onto this context's stack
    ///
    /// # Unsafety
    /// Assumes that the context's stack pointer points to a valid memory location in the current
    /// address space with room for `value`
    pub unsafe fn push_stack<T: Copy>(&mut self, value: T) {
        self.rsp -= mem::size_of::<T>();
        *(self.rsp as *mut T) = value;
    }

    /// Pop a value off of this context's stack
    ///
    /// # Unsafety
    /// Assumes that the context's stack pointer points to a valid memory location in the current
    /// address space and that the value at the top of the stack is of type `T`.
    pub unsafe fn pop_stack<T: Copy>(&mut self) -> T {
        let value = *(self.rsp as *const T);
        self.rsp += mem::size_of::<T>();
        value
    }

    /// Switch to another execution context.
    ///
    /// # Unsafety
    /// Should be pretty obvious why this is unsafe.
    ///
    /// In all seriousness, this relies on a lot of things being true, and has undefined behavior if
    /// any of those assumptions are false.
    /// * The pointers to both this context and the next must be valid in both of their page directories
    /// * The saved registers in the next context, especially the stack pointer, must be correct
    /// * The kernel must be using the System V AMD64 ABI
    #[inline(never)]
    #[naked]
    pub unsafe fn switch(&mut self, to: &mut Context) {
        context_switch(self, to);
    }

    // TODO: also have distinct get/set APIs?
}

extern "sysv64" {
    fn context_switch(from: &mut Context, to: &mut Context);
}

// Naked functions with arguments don't really work - since there's no normal prelude, they
// can't figure out how to access arguments. That means we have to do it all ourselves :/
// See the tracking issue: https://github.com/rust-lang/rust/issues/32408
// and this bug: https://github.com/rust-lang/rust/issues/34043

// TODO: PCID support for more efficient page table switches?
// TODO: floating-point support

// Offsets for struct fields:
//  0 - page_directory
//  8 - flags
// 16 - rsp
// 24 - rbx
// 32 - rbp
// 40 - r12
// 48 - r13
// 56 - r14
// 64 - r15

// self is in rdi and to is in rsi

global_asm!(r"#
    .global context_switch
context_switch:
    # Save the PML4
    movq %cr3, %rax
    movq %rax, (%rdi)

    # Update the PML4, if necessary
    movq (%rsi), %rcx
    cmpq %rax, %rcx
    jne .same_pml4
    movq %rcx, %cr3
.same_pml4:

    # Switch flags
    pushfq
    popq 8(%rdi)

    pushq 8(%rsi)
    popfq

    # Switch callee-save registers

    movq %rbx, 24(%rdi)
    movq 24(%rsi), %rbx

    movq %r12, 40(%rdi)
    movq 40(%rsi), %r12

    movq %r13, 48(%rdi)
    movq 48(%rsi), %r13

    movq %r14, 56(%rdi)
    movq 56(%rsi), %r14

    movq %r15, 64(%rdi)
    movq 64(%rsi), %r15

    movq %rbp, 32(%rdi)
    movq 32(%rsi), %rbp

    movq %rsp, 16(%rdi)
    movq 16(%rsi), %rsp

    # Switched the stack, so we're done!
    retq
#");

/// Initial function used for new contexts. This wrapper performs setup to call the desired initial
/// function with the expected ABI. It's fine to make it a naked function since it takes no arguments
#[naked]
unsafe fn context_initial_func() -> ! {
    // The convention for a new context is that the address of the first function to call goes in
    // rbx and its 4 arguments go in r12-r15. We don't currently support more than 4 arguments
    // because that should hopefully be enough and this way we don't need to explicitly put arguments
    // on the stack

    // System V ABI is that first 6 arguments go in rdi, rsi, rdx, rcx, r8, and r9
    // For some reason, calling to a register doesn't compile with AT&T syntax
    asm!("mov rdi, r12\n\t\
         mov rsi, r13\n\t\
         mov rdx, r14\n\t\
         mov rcx, r15\n\t\
         call rbx" : : : "memory" "rdi" "rsi" "rdx" "rcx" : "volatile", "intel");

    panic!("Initial context function returned");
}
