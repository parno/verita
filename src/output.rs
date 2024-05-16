use serde::{Deserialize, Serialize};

#[derive(Deserialize, Hash)]
#[serde(rename_all = "kebab-case")]
struct VerusOutputTimesMs {
    estimated_cpu_time: u64,
    total: u64,
    smt: VerusOutputSmtTimesMs,
}

#[derive(Deserialize, Hash)]
#[serde(rename_all = "kebab-case")]
struct VerusOutputSmtTimesMs {
    smt_init: u64,
    smt_run: u64,
    total: u64,
}

#[derive(Debug, Serialize, Deserialize, Hash, Clone)]
#[serde(rename_all = "kebab-case")]
struct VerusOutputVerificationResults {
    encountered_vir_error: bool,
    success: Option<bool>,
    verified: Option<u64>,
    errors: Option<u64>,
    is_verifying_entire_crate: Option<bool>,
}

#[derive(Deserialize, Hash)]
#[serde(rename_all = "kebab-case")]
struct VerusOutput {
    times_ms: VerusOutputTimesMs,
    verification_results: VerusOutputVerificationResults,
}