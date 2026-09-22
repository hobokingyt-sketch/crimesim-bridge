use crate::{Fingerprint, invalid, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind { Initialize, Update, Rollback, Repair }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase { Preparing, Ready, SourcePromoting, SourcePromoted, BuildPromoting, PairPromoted, Committed, Restoring, Recovered }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema: u32, pub transaction_id: String, pub project_id: String, pub revision: u64,
    pub source: Fingerprint, pub build: Fingerprint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Package { pub original: String, pub sha256: String }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Journal {
    pub schema: u32, pub transaction_id: String, pub kind: Kind, pub phase: Phase,
    pub project_id: String, pub target_revision: u64,
    pub before_source: Fingerprint, pub before_build: Fingerprint,
    pub after_source: Option<Fingerprint>, pub after_build: Option<Fingerprint>,
    pub before_receipt: Option<Receipt>, pub package: Option<Package>,
}

pub(crate) fn valid_id(id: &str) -> Result<()> {
    if !id.starts_with("t-") || id.len() < 6 || id.len() > 100
        || !id.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-' || c == b't') {
        return Err(invalid("Invalid transaction identity; recovery blocked"));
    }
    Ok(())
}
impl Journal {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.schema != 2 { return Err(invalid("Unsupported recovery journal; preserve workspace for repair")); }
        valid_id(&self.transaction_id)?;
        if self.project_id.is_empty() { return Err(invalid("Empty journal project identity")); }
        if let Some(r) = &self.before_receipt { valid_id(&r.transaction_id)?; }
        if !matches!(self.phase, Phase::Preparing | Phase::Recovered)
            && (self.after_source.is_none() || self.after_build.is_none()) {
            return Err(invalid("Incomplete recovery fingerprints"));
        }
        Ok(())
    }
}
