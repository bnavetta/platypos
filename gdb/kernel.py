import os

class SetupKernel(gdb.Command):
    """
    setup-kernek: Configure GDB for debugging the kernel
    """

    def __init__(self):
        super(SetupKernel, self).__init__("setup-kernel", gdb.COMMAND_USER)
        self.dont_repeat()
    
    def invoke(self, args, from_tty):
        # The kernel isn't relocatable, so we can add it directly
        kernel_exe = os.getenv("kernel_exe")

        # Remove any previously-loaded symbol file
        try:
            gdb.execute(f'remove-symbol-file {kernel_exe}')
        except Exception as _e:
            pass
        
        gdb.execute(f'add-symbol-file {kernel_exe}')

        # Tell the kernel that GDB is attached
        gdb.execute('set KERNEL_DEBUGGER_ATTACHED=1')

SetupKernel()