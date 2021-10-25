
kernel_package := "platypos_kernel"
kernel_exe := "target/riscv64gc-unknown-none-elf/debug/platypos_kernel"
kernel_bin := "target/riscv64gc-unknown-none-elf/debug/platypos_kernel.bin"
state_dir := ".state"
fw_file := "/usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.elf"
# fw_file := "../oreboot/src/mainboard/emulation/qemu-riscv/target/riscv64imac-unknown-none-elf/release/image.bin"

uroot_cmd := env_var_or_default("GOPATH", env_var("HOME") + "/go") + "/bin/u-root"
uroot_initramfs := "target/initramfs.uroot.cpio"
linuxboot_kernel := "arch/riscv/boot/Image.gz"

# Use all but 1 core for building
build_cores := `nproc --ignore=1`
linux_makeflags := "ARCH=riscv CROSS_COMPILE=riscv64-linux-gnu- -j" + build_cores

export GOOS := "linux"
export GOARCH := "riscv64"

# TODO: set up UEFI

# TODO: use LinuxBoot, since the whole idea is that you can write your own bootloader/app (esp. in Go)
# reuse their libraries to load+run an elf kernel?
# Maybe use FIT (also supported by U-Boot) https://doc.coreboot.org/lib/payloads/fit.html

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
    -nographic #-serial mon:stdio

# To add a hard drive (must exist)
#     -drive if=none,format=raw,file={{state_dir}}/hdd.dsk,id=hdd \
#    -device virtio-blk-device,scsi=off,drive=hdd \


# Build u-root userspace for LinuxBoot
u-root:
  "{{ uroot_cmd }}"  -o "{{ uroot_initramfs }}" core boot # TODO: add custom bootloader here


# Compile LinuxBoot BIOS
linuxboot:
  #!/usr/bin/env bash
  # TODO: assuming linuxboot doesn't need to be cross-compiled because we're just rearranging the image
  set -euxo pipefail
  cd linuxboot
  make BOARD=qemu KERNEL="../{{ linuxboot_kernel }}" INITRD="../{{ uroot_initramfs }}" config
  make -j{{ build_cores }}

# Reconfigure the Linux kernel
linux-config:
  #!/usr/bin/env bash
  set -euxo pipefail
  cd linux-stable
  # make ARCH=riscv LLVM=1 tinyconfig
  make {{ linux_makeflags }} tinyconfig

  ./scripts/config --set-str INITRAMFS_SOURCE "../{{ uroot_initramfs }}"
  ./scripts/config --set-str DEFAULT_HOSTNAME linuxboot
  ./scripts/config --enable EFI_BDS
  ./scripts/config --enable EPOLL
  ./scripts/config --enable FUTEX
  ./scripts/config --enable DEVTMPFS
  ./scripts/config --enable DEVTMPFS_MOUNT
  ./scripts/config --disable DEBUG_RT_MUTEXES

linux: u-root
  #!/usr/bin/env bash
  set -euxo pipefail
  cd linux-stable
  # make ARCH=riscv LLVM=1 -j{{ build_cores }}
  make {{ linux_makeflags }}

oreboot:
  make -C oreboot/src/mainboard/emulation/qemu-riscv mainboard

gdb:
  gdb -x gdb/init

addr2line +ADDRS:
  @addr2line -C -p -f -e "{{ kernel_exe }}" {{ ADDRS }}

check:
  cargo check -p {{kernel_package}}

format:
  cargo fmt --all

dependencies:
  yay -S qemu qemu-arch-extra bc riscv64-linux-gnu-gcc nasm acpica
  rustup target add riscv64gc-unknown-none-elf
  rustup component add llvm-tools-preview
  cargo install cargo-binutils # TODO: oreboot installs a specific cargo-binutils version
  go get github.com/u-root/u-root
  make -C oreboot firsttime
