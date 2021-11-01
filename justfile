
# Booting:
# Ideally, something like LinuxBoot would be great, since you can write your own loader as a Linux userspace program
# However, I couldn't get various combinations of coreboot/oreboot or LinuxBoot to work for RISC-V
# Instead, this just uses OpenSBI directly
# An alternative would be U-Boot (possibly with a FIT image), but it's less clear how to hand it a bootable file
# It's probably also doable to get UEFI set up and implement a UEFI-based loader

# Run PlatypOS
run:
  ./pos run

# To add a hard drive (must exist)
#     -drive if=none,format=raw,file={{state_dir}}/hdd.dsk,id=hdd \
#    -device virtio-blk-device,scsi=off,drive=hdd \

gdb:
  ./pos debugger

# addr2line +ADDRS:
#   @addr2line -C -p -f -e "{{ kernel_exe }}" {{ ADDRS }}

check:
  cd kernel && cargo clippy
  cd tools && cargo clippy

# Format all source code
fmt:
  cd kernel && cargo fmt
  cd tools && cargo fmt

dependencies:
  yay -S qemu qemu-arch-extra bc riscv64-linux-gnu-gcc riscv64-linux-gnu-gdb
  rustup update
  cargo install cargo-binutils
