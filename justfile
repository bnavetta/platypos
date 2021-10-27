
kernel_package := "platypos_kernel"
kernel_exe := "target/riscv64gc-unknown-none-elf/debug/platypos_kernel"
kernel_bin := "target/riscv64gc-unknown-none-elf/debug/platypos_kernel.bin"
state_dir := ".state"
fw_file := "/usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.elf"

# Booting:
# Ideally, something like LinuxBoot would be great, since you can write your own loader as a Linux userspace program
# However, I couldn't get various combinations of coreboot/oreboot or LinuxBoot to work for RISC-V
# Instead, this just uses OpenSBI directly
# An alternative would be U-Boot (possibly with a FIT image), but it's less clear how to hand it a bootable file
# It's probably also doable to get UEFI set up and implement a UEFI-based loader

# Build the kernel
kernel:
  cargo build -p {{kernel_package}}
  rust-objcopy "{{ kernel_exe }}" --binary-architecture=riscv64 --strip-all -O binary "{{ kernel_bin }}"

# Run PlatypOS
run: kernel
  @mkdir -p {{state_dir}}

  qemu-system-riscv64 \
    -machine virt \
    -cpu rv64 \
    -smp 4 \
    -m 1G \
    -device virtio-rng-device \
    -device virtio-gpu-device \
    -device virtio-net-device \
    -device virtio-tablet-device \
    -device virtio-keyboard-device \
    -bios "{{ fw_file }}" \
    -kernel "{{ kernel_bin }}" \
    -nographic \
    -serial mon:stdio

# To add a hard drive (must exist)
#     -drive if=none,format=raw,file={{state_dir}}/hdd.dsk,id=hdd \
#    -device virtio-blk-device,scsi=off,drive=hdd \

gdb:
  gdb -x gdb/init

addr2line +ADDRS:
  @addr2line -C -p -f -e "{{ kernel_exe }}" {{ ADDRS }}

check:
  cargo check -p {{kernel_package}}

format:
  cargo fmt --all

dependencies:
  yay -S qemu qemu-arch-extra bc riscv64-linux-gnu-gcc riscv64-linux-gnu-gdb
  rustup update
  cargo install cargo-binutils
