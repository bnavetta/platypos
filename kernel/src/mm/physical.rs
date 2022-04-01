//! Physical memory management (page frames)

use super::map::Region;

/// Builder for the physical memory allocator
pub struct Builder;

/// The address space may contain many small reserved/unusable memory regions.
/// To avoid fragmenting the allocator state into chunks with high bookkeeping
/// overhead, small unusable regions are merged into larger usable regions and
/// marked as unavailable.
const MAP_HOLE_THRESHOLD: usize = 4 * 1024 * 1024;

impl Builder {
    pub fn parse_memory_map<I>(&mut self, mut memory_map: I)
    where
        I: Iterator<Item = Region> + Clone,
    {
        // let map2 = memory_map.clone();

        let mut current_region = memory_map.find(|r| r.usable());
        let mut has_holes = false;
        let mut has_usable_memory = current_region.as_ref().map(|r| r.usable()).unwrap_or(false);
        for region in memory_map {
            match current_region.as_mut() {
                None => {
                    if region.usable() {
                        has_usable_memory = region.usable();
                        current_region = Some(region);
                    }
                }
                Some(cur) => {
                    let overlaps = region.start() <= cur.end();
                    if overlaps && (region.usable() || region.size() < MAP_HOLE_THRESHOLD) {
                        has_holes = has_holes || !region.usable();
                        has_usable_memory = has_usable_memory || region.usable();
                        *cur = Region::new(cur.kind(), cur.start(), region.end());
                    } else {
                        if has_usable_memory {
                            log::info!(
                                "Allocator region {}{}",
                                cur,
                                if has_holes { " with holes " } else { "" }
                            );
                        } // else skip completely-unusable regions

                        // TODO: set to None if region is unusable?
                        has_holes = !region.usable();
                        has_usable_memory = !has_holes;
                        *cur = region;
                    }
                }
            }
        }

        if let Some(region) = current_region {
            if has_usable_memory {
                log::info!(
                    "Allocator region {}{}",
                    region,
                    if has_holes { " with holes " } else { "" }
                );
            }
        }
    }
}
