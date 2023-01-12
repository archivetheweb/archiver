use serde_derive::Deserialize;
use serde_derive::Serialize;
use serde_json::Value;
use std::convert::From;
use std::fmt::Display;
use std::num::ParseIntError;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub manifest: Manifest,
    pub workers_config: WorkersConfig,
    #[serde(rename = "queues_totals")]
    pub queues_totals: QueuesTotals,
    #[serde(rename = "queues_details")]
    pub queues_details: QueuesDetails,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub git_commit_hash: String,
    pub warp_sdk_config: WarpSdkConfig,
    pub evaluation_options: EvaluationOptions,
    pub owner: String,
    pub wallet_address: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WarpSdkConfig {
    #[serde(rename = "warp-contracts")]
    pub warp_contracts: String,
    #[serde(rename = "warp-contracts-lmdb")]
    pub warp_contracts_lmdb: String,
    #[serde(rename = "warp-contracts-evaluation-progress-plugin")]
    pub warp_contracts_evaluation_progress_plugin: String,
    #[serde(rename = "warp-contracts-plugin-nlp")]
    pub warp_contracts_plugin_nlp: String,
    #[serde(rename = "warp-contracts-plugin-ethers")]
    pub warp_contracts_plugin_ethers: String,
    #[serde(rename = "warp-contracts-plugin-signature")]
    pub warp_contracts_plugin_signature: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationOptions {
    #[serde(rename = "useVM2")]
    pub use_vm2: bool,
    pub max_call_depth: i64,
    pub max_interaction_evaluation_time_seconds: i64,
    pub allow_big_int: bool,
    pub unsafe_client: String,
    pub internal_writes: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkersConfig {
    pub register: i64,
    pub update: i64,
    pub job_id_refresh_seconds: i64,
    pub max_failures: i64,
    pub max_state_size_b: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuesTotals {
    pub update: Update,
    pub register: Register,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    pub active: i64,
    pub waiting: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Register {
    pub active: i64,
    pub waiting: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuesDetails {
    pub update: Update2,
    pub register: Register2,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Update2 {
    pub active: Vec<Value>,
    pub waiting: Vec<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Register2 {
    pub active: Vec<Value>,
    pub waiting: Vec<Value>,
}

#[derive(Debug)]
pub enum WarpDREError {
    ArgumentError { arg: String },
    URLError { url: String },
    IOError(std::io::Error),
    ReqwestError(reqwest::Error),
    ParseIntError(ParseIntError),
}

impl Display for WarpDREError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WarpDREError::ArgumentError { arg } => write!(f, "argument not valid: {}", arg),
            WarpDREError::IOError(e) => write!(f, "io: {}", e),
            WarpDREError::ReqwestError(e) => write!(f, "reqwest: {}", e),
            WarpDREError::URLError { url } => write!(f, "invalid url: {}", url),
            WarpDREError::ParseIntError(e) => write!(f, "parse int error: {}", e),
        }
    }
}

impl WarpDREError {
    pub fn invalid_argument(arg: &str) -> WarpDREError {
        WarpDREError::ArgumentError {
            arg: arg.to_string(),
        }
    }
    pub fn invalid_url(url: &str) -> WarpDREError {
        WarpDREError::URLError {
            url: url.to_string(),
        }
    }
}

impl From<std::io::Error> for WarpDREError {
    fn from(e: std::io::Error) -> Self {
        WarpDREError::IOError(e)
    }
}

impl From<reqwest::Error> for WarpDREError {
    fn from(e: reqwest::Error) -> Self {
        WarpDREError::ReqwestError(e)
    }
}

impl From<std::num::ParseIntError> for WarpDREError {
    fn from(e: std::num::ParseIntError) -> Self {
        WarpDREError::ParseIntError(e)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlacklistItem {
    #[serde(rename = "contract_tx_id")]
    pub contract_tx_id: String,
    pub failures: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cached {
    pub cached_contracts: i64,
    pub ids: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorsItem {
    #[serde(rename = "contract_tx_id")]
    pub contract_tx_id: String,
    #[serde(rename = "evaluation_options")]
    pub evaluation_options: String,
    #[serde(rename = "sdk_config")]
    pub sdk_config: String,
    #[serde(rename = "job_id")]
    pub job_id: String,
    pub failure: String,
    pub timestamp: String,
}
