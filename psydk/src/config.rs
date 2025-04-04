#[derive(Debug, Clone)]
pub struct ExperimentConfig {
    /// pedantic mode
    pub pedantic: bool,
    /// debug mode
    pub debug: bool,
}

impl Default for ExperimentConfig {
    fn default() -> Self {
        Self {
            pedantic: true,
            debug: false,
        }
    }
}
