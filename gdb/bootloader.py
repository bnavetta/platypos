import os

import pefile

"""
This adds debug symbols for the UEFI bootloader application. It relies on the bootloader's cooperation (see `wait_for_debugger`).
The overall approach is inspired by https://xitan.me/posts/rust-uefi-runtime-driver/, but instead of scanning memory to find the base address,
it has the bootloader report it via registers.
"""
# TODO: Take a leaf out of https://fasterthanli.me/series/making-our-own-executable-packer/part-9 and rewrite this in Rust using goblin


def get_loader_base():
    """
    Gets the base address that the UEFI loader is loaded at.
    """
    r13 = gdb.selected_frame().read_register('r13')
    return int(r13)


def get_loader_sections(loader_file, base_address):
    exe = pefile.PE(loader_file)
    sections = dict()
    for section in exe.sections:
        name = section.Name.decode().rstrip('\x00')
        address = section.VirtualAddress + base_address
        if name[0] != '/':
            print(f'  Section {name}: 0x{address:02x}')
            sections[name] = address
    return sections


class SetupBootloader(gdb.Command):
    """
    setup-bootloader: Configure GDB for debugging the bootloader
    """

    def __init__(self):
        super(SetupBootloader, self).__init__("setup-bootloader", gdb.COMMAND_USER)
        self.dont_repeat()
    
    def invoke(self, args, from_tty):
        loader_exe = os.getenv("loader_exe")
        base_address = get_loader_base()
        print(f'UEFI loader {loader_exe} at 0x{base_address:02x}')
        sections = get_loader_sections(loader_exe, base_address)

        # Remove any previously-loaded symbol file
        try:
            gdb.execute(f'remove-symbol-file {loader_exe}')
        except Exception as _e:
            pass
        
        section_mappings = ' '.join(f'-s {name} {address}' for name, address in sections.items() if name != '.text')
        textaddr = sections['.text']
        gdb.execute(f'add-symbol-file {loader_exe} {textaddr} {section_mappings}')

        # Tell the bootloader that GDB is attached
        gdb.execute('set DEBUGGER_ATTACHED=1')

SetupBootloader()