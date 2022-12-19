use platypos_hal as hal;

#[derive(Debug, Clone, Copy)]
pub struct Topology;

impl hal::topology::Topology for Topology {
    const MAX_PROCESSORS: u16 = 16;

    fn current_processor(&self) -> hal::topology::ProcessorId {
        0
    }
}

pub static INSTANCE: Topology = Topology;
