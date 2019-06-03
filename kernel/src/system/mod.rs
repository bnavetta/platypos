//! The system module and submodules support various platform features and components we kinda just
//! need to deal with. This includes hardware or processor components which are used in many places,
//! like the APIC, and don't quite belong in any of them.

pub mod apic;
pub mod gdt;
pub mod pic;
