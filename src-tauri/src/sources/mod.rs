mod heimao;

pub use heimao::HeimaoAdapter;

use crate::models::Partner;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub partner: Partner,
    /// subject terms = lexicon
    pub subject_terms: Vec<String>,
    pub window_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HeimaoMetrics {
    pub complaint_count_30d: u32,
    pub complaint_count_prev_30d: u32,
    pub resolve_rate: f64,
}

impl HeimaoMetrics {
    pub fn growth_ratio(&self) -> f64 {
        if self.complaint_count_prev_30d == 0 {
            if self.complaint_count_30d > 0 {
                1.0
            } else {
                0.0
            }
        } else {
            (self.complaint_count_30d as f64 - self.complaint_count_prev_30d as f64)
                / self.complaint_count_prev_30d as f64
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawHit {
    pub source_id: String,
    pub title: String,
    pub summary: String,
    pub url: String,
    pub event_time: String,
    pub body: String,
    /// A=官方文书 B=媒体实锤 C=网传
    pub credibility: String,
    pub related_party_term: Option<String>,
    pub heimao: Option<HeimaoMetrics>,
}

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("源站受阻: {0}")]
    Blocked(String),
    #[error("适配器错误: {0}")]
    #[allow(dead_code)]
    Other(String),
}

pub trait SourceAdapter: Send + Sync {
    fn source_id(&self) -> &'static str;
    fn search(&self, req: &SearchRequest) -> Result<Vec<RawHit>, SourceError>;
}

/// 主体词 AND 风险关键词 双条件匹配。
pub fn dual_match(text: &str, subject_terms: &[String], risk_keywords: &[String]) -> bool {
    let lower = text.to_lowercase();
    let has_subject = subject_terms.iter().any(|t| {
        let t = t.trim();
        !t.is_empty() && lower.contains(&t.to_lowercase())
    });
    let has_risk = risk_keywords.iter().any(|k| {
        let k = k.trim();
        !k.is_empty() && lower.contains(&k.to_lowercase())
    });
    has_subject && has_risk
}

pub fn evidence_hash(source: &str, url: &str, title: &str, body: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    hasher.update(b"|");
    hasher.update(url.as_bytes());
    hasher.update(b"|");
    hasher.update(title.as_bytes());
    hasher.update(b"|");
    hasher.update(body.chars().take(200).collect::<String>().as_bytes());
    hex::encode(hasher.finalize())
}

pub fn all_risk_keywords(keywords_yaml: &str) -> Vec<String> {
    #[derive(Deserialize)]
    struct Root {
        categories: std::collections::HashMap<String, Cat>,
    }
    #[derive(Deserialize)]
    struct Cat {
        words: Vec<String>,
    }
    let Ok(root) = serde_yaml::from_str::<Root>(keywords_yaml) else {
        return Vec::new();
    };
    root.categories
        .into_values()
        .flat_map(|c| c.words)
        .collect()
}
