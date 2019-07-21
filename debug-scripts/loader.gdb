# GDB script with settings for debugging the UEFI OS loader application
# Also see https://wiki.osdev.org/Debugging_UEFI_applications_with_GDB

set arch i386:x86-64:intel
target remote localhost:1234
# Load the symbol file with relocations (have to check that the relocation address matches the one printed out)
add-symbol-file target/x86_64-unknown-uefi/debug/platypos_loader.efi 0xd487000 -s .data 0xd487000

break platypos_loader::loader::exit_boot_services