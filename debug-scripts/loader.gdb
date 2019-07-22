# GDB script with settings for debugging the UEFI OS loader application
# Also see https://wiki.osdev.org/Debugging_UEFI_applications_with_GDB

set arch i386:x86-64:intel
target remote localhost:1234
# Load the symbol file with relocations (have to check that the relocation address matches the one printed out)
# Base: 0xd3f4000
add-symbol-file target/x86_64-unknown-uefi/debug/platypos_loader.efi 0xd3f5000 -s .data 0xd40a000 -s .rdata 0xd404000

hbreak handoff.rs:29