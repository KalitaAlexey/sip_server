use crate::via_branch_generator::ViaBranchGenerator;
use async_std::sync::Mutex;

#[derive(Default)]
pub struct Utils {
    via_branch_generator: Mutex<ViaBranchGenerator>,
}

impl Utils {
    pub fn new() -> Self {
        Self {
            via_branch_generator: Mutex::new(ViaBranchGenerator::new()),
        }
    }

    pub async fn via_branch(&self) -> String {
        self.via_branch_generator.lock().await.branch()
    }
}
