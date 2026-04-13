use serde::{Deserialize, Serialize};
use atriolum_protocol::Level;

/// Filter parameters for listing events.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventFilter {
    pub level: Option<Level>,
    pub limit: Option<usize>,
    pub since: Option<String>,
}
