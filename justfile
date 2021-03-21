
boot_dir := "target/boot"

loader_package := "platypos_loader"
# loader_target := "x86_64-unknown-uefi"
loader_target := "x86_64-none-efi.json"
loader_target_name := "x86_64-none-efi"
export loader_exe := "target/" + loader_target_name + "/debug/" + loader_package + ".efi"

kernel_package := "platypos_kernel"
kernel_target := "x86_64-platypos.json"
kernel_target_name := "x86_64-platypos"
export kernel_exe := "target/" + kernel_target_name + "/debug/" + kernel_package

# OVMF
ovmf_fw_path := "/usr/share/ovmf/x64/OVMF_CODE.fd"
ovmf_vars_path := "/usr/share/ovmf/x64/OVMF_VARS.fd"

# Options
debug_loader := "false"
debug_kernel := "false"

# Build the bootloader
loader:
  cargo build -p {{loader_package}} --target {{loader_target}} {{ if debug_loader == "true" { "--features gdb" } else { "" } }}

# Build the kernel
kernel:
  cargo build -p {{kernel_package}} --target {{kernel_target}} {{ if debug_kernel == "true" { "--features gdb" } else { "" } }}

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
    -d int,cpu_reset \
    {{ if debug_loader == "true" { "-s" } else { if debug_kernel == "true" { "-s" } else { "" } } }} \
    -serial stdio

gdb:
  gdb -x gdb/init

addr2line +ADDRS:
  @addr2line -C -p -f -e "{{ kernel_exe }}" {{ ADDRS }}

check:
  cargo check -p {{loader_package}} --target {{loader_target}}
  cargo check -p {{kernel_package}} --target {{kernel_target}}

format:
  cargo fmt --all

dependencies:
  yay -S qemu edk2-ovmf
  pip3 install --user pefile