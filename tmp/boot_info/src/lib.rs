#![no_std]

use core::{fmt, slice, str};

use x86_64::PhysAddr;

/// Boot information passed to the kernel
#[derive(Debug)]
// These attributes are for FFI safety, see https://github.com/rust-osdev/bootloader/commit/86d1db72fd334e34dcfc17c78540b8365a974199
#[repr(C)]
#[non_exhaustive]
pub struct BootInfo {
    magic: u64,

    rsdp_address: Optional<u64>,

    memory_map: Slice<MemoryRegion>,
}

impl BootInfo {
    pub const MAGIC: u64 = u64::from_le_bytes(*b"PLATYPOS");

    pub fn new(rsdp_address: Option<PhysAddr>, memory_map: &'static [MemoryRegion]) -> BootInfo {
        BootInfo {
            magic: BootInfo::MAGIC,
            rsdp_address: rsdp_address.map(PhysAddr::as_u64).into(),
            memory_map: memory_map.into(),
        }
    }

    pub fn assert_valid(&self) {
        assert_eq!(self.magic, BootInfo::MAGIC, "Invalid boot magic");
    }

    pub fn rsdp_address(&self) -> Option<PhysAddr> {
        let rsdp_opt: Option<u64> = self.rsdp_address.into();
        rsdp_opt.map(PhysAddr::new)
    }

    pub fn memory_map(&self) -> &'static [MemoryRegion] {
        self.memory_map.as_slice()
    }
}

impl fmt::Display for BootInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Boot Info:\n")?;
        let magic_bytes = self.magic.to_le_bytes();
        writeln!(f, "  Magic: {:#x} ({})", self.magic, str::from_utf8(&magic_bytes).unwrap())?;
        write!(f, "  RSDP Address: ")?;
        match self.rsdp_address() {
            Some(addr) => writeln!(f, "{:#x}", addr.as_u64()),
            None => writeln!(f, "<unknown>"),
        }?;
        writeln!(f, "  Memory Map:")?;
        for region in self.memory_map() {
            let size = region.end - region.start;
            writeln!(f, "    * {:#010x} - {:#010x}: {:?}, {} bytes, {} pages", region.start, region.end, region.kind, size, size / 4096)?;
        }
        Ok(())
    }
}

/// FFI-safe [`Option`] type. See [this thread](https://users.rust-lang.org/t/option-is-ffi-safe-or-not/29820/6) for context.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(C)]
enum Optional<T> {
    Some(T),
    None,
}

impl <T> From<Option<T>> for Optional<T> {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => Optional::Some(v),
            None => Optional::None,
        }
    }
}

impl <T> Into<Option<T>> for Optional<T> {
    fn into(self) -> Option<T> {
        match self {
            Optional::Some(v) => Some(v),
            Optional::None => None,
        }
    }
}

/// FFI-safe static immutable slice
#[repr(C)]
struct Slice<T: 'static> {
    ptr: *const T,
    length: usize,
}

impl <T: 'static> Slice<T> {
    #[allow(dead_code)]
    pub fn empty() -> Slice<T> {
        Slice {
            ptr: ::core::ptr::null(),
            length: 0
        }
    }

    fn as_slice(&self) -> &'static [T] {
        // Safety: Slice can only be created from static slices, so the pointer should still be valid
        unsafe { slice::from_raw_parts(self.ptr, self.length) }
    }
}

impl <T: 'static> From<&'static [T]> for Slice<T> {
    fn from(slice: &'static [T]) -> Slice<T> {
        Slice {
            ptr: slice.as_ptr(),
            length: slice.len(),
        }
    }
}

impl <T: fmt::Debug + 'static> fmt::Debug for Slice<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.as_slice().fmt(f)
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct MemoryRegion {
    start: u64,
    end: u64,
    kind: MemoryKind,
}

impl MemoryRegion {
    pub fn new(start: PhysAddr, end: PhysAddr, kind: MemoryKind) -> MemoryRegion {
        MemoryRegion {
            start: start.as_u64(),
            end: end.as_u64(),
            kind,
        }
    }

    pub fn kind(&self) -> MemoryKind {
        self.kind
    }

    pub fn start(&self) -> PhysAddr {
        PhysAddr::new(self.start)
    }

    pub fn end(&self) -> PhysAddr {
        PhysAddr::new(self.end)
    }

    /// The size of this region, in bytes
    pub fn len(&self) -> u64 {
        self.end - self.start
    }

    /// The size of this region, in physical memory frames
    pub fn frames(&self) -> u64 {
        self.len() / 4096
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MemoryKind {
    /// Conventional, usable memory
    Usable,
    /// Memory containing the kernel
    Kernel,
    /// Memory allocated by the bootloader for the kernel that can later be reclaimed. This includes the kernel's initial page table.
    KernelReclaimable,
    /// Memory containing ACPI tables. This memory can be reused if the tables are no longer needed.
    AcpiTables,
    /// Memory used by UEFI runtime services
    UefiRuntime,
    /// Non-volatile / persistent memory
    NonVolatile,
}