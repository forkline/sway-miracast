pub struct Daemon;

impl Daemon {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        todo!("orchestration and session lifecycle")
    }
}