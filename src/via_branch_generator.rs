#[derive(Default)]
pub struct ViaBranchGenerator {
    counter: u64,
}

impl ViaBranchGenerator {
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    /// Returns a next unique branch
    pub fn branch(&mut self) -> String {
        // As per https://tools.ietf.org/html/rfc3261#section-8.1.1.7 any branch must start with "z9hG4bK"
        let branch = format!("z9hG4bK-{}", self.counter);
        self.counter += 1;
        branch
    }
}
