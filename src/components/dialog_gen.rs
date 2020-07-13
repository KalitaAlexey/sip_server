use rand::RngCore;

#[derive(Debug, Default)]
pub struct DialogGen;

impl DialogGen {
    pub fn new() -> Self {
        Self
    }

    pub fn call_id(&self) -> String {
        let v1 = rand::rngs::OsRng.next_u64();
        let v2 = rand::rngs::OsRng.next_u64();
        format!("{:x}-{:x}", v1, v2)
    }

    pub fn tag(&self) -> String {
        format!("{:x}", rand::rngs::OsRng.next_u64())
    }
}
