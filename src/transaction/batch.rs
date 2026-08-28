pub struct Batch {
    size: u32,
}

impl Batch {
    pub fn new(size: u32) -> Self {
        Self { size }
    }

    pub fn size(&self) -> u32 {
        self.size
    }
}
