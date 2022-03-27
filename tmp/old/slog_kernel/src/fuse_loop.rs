//! Error-swallowing drain that loops infinitely. This allows logging from panic handlers.

use slog::{Drain, Never, OwnedKVList, Record};
use x86_64_ext::instructions::hlt_loop;

pub struct FuseLoop<D: Drain> {
    inner: D,
}

impl<D: Drain> FuseLoop<D> {
    pub fn new(drain: D) -> FuseLoop<D> {
        FuseLoop { inner: drain }
    }
}

impl<D: Drain> Drain for FuseLoop<D> {
    type Ok = D::Ok;
    type Err = Never;

    fn log(&self, record: &Record, values: &OwnedKVList) -> Result<D::Ok, Never> {
        self.inner.log(record, values).map_err(|_| hlt_loop())
    }
}
