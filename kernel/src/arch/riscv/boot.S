# Based on https://osblog.stephenmarz.com/ch1.html
# However, we assume we're booted by OpenSBI, and so this code only runs on one hart

.option norvc
.section .data

.section .text.init
.global _start

_start:
  #li t0, 0x5555
  #li t1, 0x100000
  #sw t0, (t1)
  #j 3f # bf73ddf0


  # If we're not running on hart 0, wait
  #bnez a0, 3f
  # Disable virtual memory
  csrw satp, zero
.option push
.option norelax
  # Initialize the global variable pointer
  la gp, _global_pointer
.option pop
  # Zero out uninitialized global variables (BSS)
  la t0, _bss_start
  la t1, _bss_end
  bgeu t0, t1, 2f
1:
  # Writes zeros in 8-byte chunks
  sd zero, (t0)
  addi t0, t0, 8
  bltu t0, t1, 1b
2:
  # Use the stack defined in our linker script
  la sp, _stack_end

  call kmain
  j 3f

  # TODO: use supervisor-mode registers instead
  # Set bits 12:11 to 11 - machine mode
  # and bits 7 and 3 to enable coarse interrupts
  li t0, (0b11 << 11) | (1 << 7) | (1 << 3)
  csrw mstatus, t0
  la t1, kmain
  csrw mepc, t1
  # TODO: trap handler
  #la t2, asm_trap_vector
  #csrw mtvec, t2
  # Enable particular interrupts:
  # 3: MSIE - machine-level software interrupts
  # 7: MTIE - machine-level timer interrupts
  # 11: MEIE - machine-level external interrupts
  li t3, (1 << 3) | (1 << 7) | (1 << 11)
  csrw mie, t3
  la ra, 3f # If kmain returns, then loop
  mret # Use a trap return (mret) to set CPU status registers back to what we expect
3:
  # In RISC-V assembly, numeric labels are for local jumps
  # The f and b suffixes specify whether it's a forward or backward reference
  # https://github.com/riscv-non-isa/riscv-asm-manual/blob/master/riscv-asm.md
  wfi
  j 3b