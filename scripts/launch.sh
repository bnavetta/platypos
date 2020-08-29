#!/bin/bash
set -euo pipefail

function info {
    echo -e "\e[1;36m$@\e[0m"
}

ESP_DIR="target/esp"
QEMU_SOCKET="target/qemu-monitor.sock"

rm -rf "$ESP_DIR"
rm -f "$QEMU_SOCKET"

# Create the EFI system partition
info "Creating EFI system partition in $ESP_DIR..."

mkdir -p "$ESP_DIR/EFI/Boot"
# TODO: figure out if debug or release
cp target/x86_64-unknown-uefi/debug/platypos_loader.efi "$ESP_DIR/EFI/Boot/BootX64.efi"
cp target/x86_64-os/debug/platypos_kernel "$ESP_DIR/platypos_kernel"

info "Starting tmux"
TMUX_SESSION="platypos"
tmux new-session -d -s "$TMUX_SESSION"

function cleanup {
    tmux kill-session -t "$TMUX_SESSION" || true
}

trap cleanup EXIT

qemu_args=(
    'qemu-system-x86_64'

    # TODO: constant TSC
    '-machine' 'q35,accel=kvm:tcg'

    # OVMF (EFI firmware)
    '-drive' 'if=pflash,format=raw,file=/usr/share/ovmf/x64/OVMF_CODE.fd,readonly=on'
    '-drive' 'if=pflash,format=raw,file=/usr/share/ovmf/x64/OVMF_VARS.fd,readonly=on'

    # Emulated EFI system partition
    '-drive' "format=raw,file=fat:rw:$ESP_DIR"
    
    # Allow writing to port 0xf4 to exit QEMU
    '-device' 'isa-debug-exit,iobase=0xf4,iosize=0x04'

    # Redirect the serial port and UEFI stdout to the terminal
    '-serial' 'stdio'

    # Start QEMU monitor on a socket for listening in another window
    '-monitor' "unix:$QEMU_SOCKET,server,nowait"

    # Amount of memory, in MiB
    '-m' '1024'

    # TODO: QEMU debugging
    '-s'
)

qemu_cmd="$(IFS=' ' echo ${qemu_args[*]})"

info "QEMU command: $qemu_cmd"

tmux rename-window -t 0 'QEMU'
tmux send-keys -t 'QEMU' "$qemu_cmd" C-m

# Wait for QEMU to start before starting socat
while [ ! -S "$QEMU_SOCKET" ]; do
    sleep 1
done

tmux new-window -t "$TMUX_SESSION:1" -n 'QEMU Monitor'
tmux send-keys -t 'QEMU Monitor' "socat - unix-connect:$QEMU_SOCKET" C-m

tmux attach-session -t "$TMUX_SESSION:0"
