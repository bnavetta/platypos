# x86-64 Address Space

|    Start Address    |     End Address    |   Offset  |   Size  | Description |
|---------------------|--------------------|-----------|---------|-------------|
| 0x00000000_00000000 | 0x00007fff_fffffff |       0 B | 128 TiB | Low virtual memory (user-space eventually) |
| 0x00008000_00000000 | 0xffff7fff_fffffff |  +128 TiB |  16 EiB | Non-canonical addresses
| Map of physical memory
| Kernel

Offset is relative to start or end of address space, depending on if the region is in high or low memory