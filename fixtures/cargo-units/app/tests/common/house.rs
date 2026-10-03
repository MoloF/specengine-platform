//! Two adjacent impls in a shared unit: equal `qpath`s, both flagged
//! `duplicate` (a shared file is analysed once, so the pair is a real one).

pub struct House;

impl House {
    pub fn open(&self) {}
}

impl House {
    pub fn close(&self) {}
}
