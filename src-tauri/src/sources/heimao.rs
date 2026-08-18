use super::{RawHit, SearchRequest, SourceAdapter, SourceError};

/// 黑猫投诉适配器。真实采集通道尚未接入，禁止用演示数据充数。
pub struct HeimaoAdapter {
    pub live: bool,
}

impl Default for HeimaoAdapter {
    fn default() -> Self {
        Self { live: false }
    }
}

impl SourceAdapter for HeimaoAdapter {
    fn source_id(&self) -> &'static str {
        "heimao"
    }

    fn search(&self, _req: &SearchRequest) -> Result<Vec<RawHit>, SourceError> {
        Ok(Vec::new())
    }
}
