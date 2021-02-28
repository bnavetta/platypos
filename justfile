
boot_dir := "target/boot"

loader_package := "platypos_loader"
loader_target := "x86_64-unknown-uefi"
loader_exe := "target/" + loader_target + "/debug/" + loader_package + ".efi"

kernel_package := "platypos_kernel"
kernel_target := "x86_64-platypos.json"
kernel_exe := "target/x86_64-platypos/debug/" + kernel_package

# OVMF
ovmf_fw_path := "/usr/share/ovmf/x64/OVMF_CODE.fd"
ovmf_vars_path := "/usr/share/ovmf/x64/OVMF_VARS.fd"

# Build the bootloader
loader:
  cargo build -p {{loader_package}} --target {{loader_target}}

# Build the kernel
kernel:
  cargo build -p {{kernel_package}} --target {{kernel_target}}

# Prepare the UEFI boot directory (ESP)
@boot_dir: loader kernel
  rm -rf {{boot_dir}}
  mkdir -p {{boot_dir}}/EFI/BOOT
  cp {{loader_exe}} {{boot_dir}}/EFI/BOOT/BootX64.efi
  cp {{kernel_exe}} {{boot_dir}}/{{kernel_package}}

# Run PlatypOS
run: boot_dir
  qemu-system-x86_64 \
    -nodefaults \
    -vga std \
    -machine q35,accel=kvm:tcg \
    -m 128M \
    -drive if=pflash,format=raw,readonly,file={{ovmf_fw_path}} \
    -drive if=pflash,format=raw,readonly,file={{ovmf_vars_path}} \
    -drive format=raw,file=fat:rw:{{boot_dir}} \
    -serial stdio

check:
  cargo check -p {{loader_package}} --target {{loader_target}}
  cargo check -p {{kernel_package}} --target {{kernel_target}}

format:
  cargo fmt --all