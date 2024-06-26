use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::maven_data::MavenData;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SimpleLib {
    pub jar: PathBuf,
}

impl From<MavenData> for SimpleLib {
    fn from(value: MavenData) -> Self {
        Self { jar: value.path }
    }
}
