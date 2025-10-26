use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Note {
    // 序号(从0开始，0-17)
    pub k: i32,
    // 按键时间(毫秒)
    pub h: i32,
    // 释放时间(毫秒)，若为短按则为-1
    pub t: i32,
}

impl Note {
    pub fn new(k: i32, h: i32, t: i32) -> Self {
        Note { k, h, t }
    }
}

impl PartialOrd for Note {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Note {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.h.cmp(&other.h) {
            Ordering::Equal => self.k.cmp(&other.k),
            ord => ord,
        }
    }
}

pub struct NoteComparerByT;

impl NoteComparerByT {
    pub fn cmp(a: &Note, b: &Note) -> Ordering {
        a.t.cmp(&b.t)
    }
}