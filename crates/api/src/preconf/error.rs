// Modified from /builder/error.rs
// Intended for PreconfAPI
// TODO: cleanup and adding specific errors

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use ethereum_consensus::{
    primitives::{BlsPublicKey, Bytes32, Hash32},
    ssz::{self, prelude::*},
};
use helix_common::simulator::BlockSimError;
use helix_datastore::error::AuctioneerError;

#[derive(Debug, thiserror::Error)]
pub enum PreconfApiError {
    #[error("hyper error: {0}")]
    HyperError(#[from] hyper::Error),

    #[error("axum error: {0}")]
    AxumError(#[from] axum::Error),

    #[error("serde decode error: {0}")]
    SerdeDecodeError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("ssz deserialize error: {0}")]
    SszDeserializeError(#[from] ssz::prelude::DeserializeError),

    #[error("failed to decode header-submission")]
    FailedToDecodeHeaderSubmission,

    #[error("payload too large. max size: {max_size}, size: {size}")]
    PayloadTooLarge { max_size: usize, size: usize },

    #[error("submission for past slot. current slot: {current_slot}, submission slot: {submission_slot}")]
    SubmissionForPastSlot { current_slot: u64, submission_slot: u64 },

    #[error("builder blacklisted. pubkey: {pubkey:?}")]
    BuilderBlacklisted { pubkey: BlsPublicKey },

    #[error("incorrect timestamp. got: {got}, expected: {expected}")]
    IncorrectTimestamp { got: u64, expected: u64 },

    #[error("could not find proposer duty for slot")]
    ProposerDutyNotFound,

    #[error("invalid api key")]
    InvalidApiKey,

    #[error("payload attributes not yet known")]
    PayloadAttributesNotYetKnown,

    #[error("payload slot mismatches with current payload attributes slot. got: {got}, expected: {expected}")]
    PayloadSlotMismatchWithPayloadAttributes { got: u64, expected: u64 },

    #[error("block hash mismatch. message: {message:?}, payload: {payload:?}")]
    BlockHashMismatch { message: Hash32, payload: Hash32 },

    #[error("parent hash mismatch. message: {message:?}, payload: {payload:?}")]
    ParentHashMismatch { message: Hash32, payload: Hash32 },

    #[error("fee recipient mismatch. got: {got:?}, expected: {expected:?}")]
    FeeRecipientMismatch { got: ByteVector<20>, expected: ByteVector<20> },

    #[error("proposer public key mismatch. got: {got:?}, expected: {expected:?}")]
    ProposerPublicKeyMismatch { got: BlsPublicKey, expected: BlsPublicKey },

    #[error("slot mismatch. got: {got}, expected: {expected}")]
    SlotMismatch { got: u64, expected: u64 },

    #[error("zero value block")]
    ZeroValueBlock,

    #[error("missing withdrawls")]
    MissingWithdrawls,

    #[error("invalid withdrawls root")]
    InvalidWithdrawlsRoot,

    #[error("missing withdrawls root")]
    MissingWithdrawlsRoot,

    #[error("withdrawls root mismatch. got: {got:?}, expected: {expected:?}")]
    WithdrawalsRootMismatch { got: Hash32, expected: Hash32 },

    #[error("signature verification failed")]
    SignatureVerificationFailed,

    #[error("payload already delivered")]
    PayloadAlreadyDelivered,

    #[error("already processing newer payload")]
    AlreadyProcessingNewerPayload,

    #[error("accepted bid below floor, skipped validation")]
    BidBelowFloor,

    #[error("block validation error: {0:?}")]
    BlockValidationError(#[from] BlockSimError),

    #[error("internal error")]
    InternalError,

    #[error("datastore error: {0}")]
    AuctioneerError(#[from] AuctioneerError),

    #[error("incorrect prev_randao - got: {got:?}, expected: {expected:?}")]
    PrevRandaoMismatch { got: Bytes32, expected: Bytes32 },

    #[error("block already received: {block_hash:?}")]
    DuplicateBlockHash { block_hash: Hash32 },

    #[error(
        "not enough optimistic collateral. builder_pub_key: {builder_pub_key:?}. 
        collateral: {collateral:?}, collateral required: {collateral_required:?}"
    )]
    NotEnoughOptimisticCollateral {
        builder_pub_key: BlsPublicKey,
        collateral: U256,
        collateral_required: U256,
        is_optimistic: bool,
    },

    #[error("builder is not optimistic. builder_pub_key: {builder_pub_key:?}")]
    BuilderNotOptimistic { builder_pub_key: BlsPublicKey },

    #[error("builder not in proposer's trusted list: {proposer_trusted_builders:?}")]
    BuilderNotInProposersTrustedList { proposer_trusted_builders: Vec<String> },

    #[error("V2 submissions invalid if proposer requires regional filtering")]
    V2SubmissionsInvalidIfProposerRequiresRegionalFiltering,
}

impl IntoResponse for PreconfApiError {
    fn into_response(self) -> Response {
        match self {
            PreconfApiError::SerdeDecodeError(err) => {
                (StatusCode::BAD_REQUEST, format!("Serde decode error: {err}")).into_response()
            },
            PreconfApiError::IOError(err) => {
                (StatusCode::BAD_REQUEST, format!("IO error: {err}")).into_response()
            },
            PreconfApiError::SszDeserializeError(err) => {
                (StatusCode::BAD_REQUEST, format!("SSZ deserialize error: {err}")).into_response()
            },
            PreconfApiError::FailedToDecodeHeaderSubmission => {
                (StatusCode::BAD_REQUEST, "Failed to decode header submission").into_response()
            },
            PreconfApiError::HyperError(err) => {
                (StatusCode::BAD_REQUEST, format!("Hyper error: {err}")).into_response()
            },
            PreconfApiError::AxumError(err) => {
                (StatusCode::BAD_REQUEST, format!("Axum error: {err}")).into_response()
            },
            PreconfApiError::PayloadTooLarge{ max_size, size } => {
                (StatusCode::BAD_REQUEST, format!("Payload too large. max size: {max_size}, size: {size}")).into_response()
            },
            PreconfApiError::SubmissionForPastSlot { current_slot, submission_slot } => {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Submission for past slot. current slot: {current_slot}, submission slot: {submission_slot}"),
                ).into_response()
            },
            PreconfApiError::BuilderBlacklisted{ pubkey } => {
                (StatusCode::BAD_REQUEST, format!("Builder blacklisted. pubkey: {pubkey:?}")).into_response()
            },
            PreconfApiError::IncorrectTimestamp { got, expected } => {
                (StatusCode::BAD_REQUEST, format!("Incorrect timestamp. got: {got}, expected: {expected}")).into_response()
            },
            PreconfApiError::ProposerDutyNotFound => {
                (StatusCode::BAD_REQUEST, "Could not find proposer duty for slot").into_response()
            },
            PreconfApiError::PayloadSlotMismatchWithPayloadAttributes { got, expected } => {
                (StatusCode::BAD_REQUEST, format!("payload slot mismatches with current payload attributes slot. got: {got}, expected: {expected}")).into_response()
            },
            PreconfApiError::BlockHashMismatch { message, payload } => {
                (StatusCode::BAD_REQUEST, format!("Block hash mismatch. message: {message:?}, payload: {payload:?}")).into_response()
            },
            PreconfApiError::ParentHashMismatch { message, payload } => {
                (StatusCode::BAD_REQUEST, format!("Parent hash mismatch. message: {message:?}, payload: {payload:?}")).into_response()
            },
            PreconfApiError::ProposerPublicKeyMismatch { got, expected } => {
                (StatusCode::BAD_REQUEST, format!("Proposer public key mismatch. got: {got:?}, expected: {expected:?}")).into_response()
            },
            PreconfApiError::ZeroValueBlock => {
                (StatusCode::BAD_REQUEST, "Zero value block").into_response()
            },
            PreconfApiError::SignatureVerificationFailed => {
                (StatusCode::BAD_REQUEST, "Signature verification failed").into_response()
            },
            PreconfApiError::PayloadAlreadyDelivered => {
                (StatusCode::BAD_REQUEST, "Payload already delivered").into_response()
            },
            PreconfApiError::BidBelowFloor => {
                (StatusCode::ACCEPTED, "Bid below floor, skipped validation").into_response()
            },
            PreconfApiError::InvalidApiKey => {
                (StatusCode::UNAUTHORIZED, "Invalid api key").into_response()
            },
            PreconfApiError::InternalError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
            },
            PreconfApiError::BlockValidationError(err) => {
                match err {
                    BlockSimError::Timeout => {
                        (StatusCode::GATEWAY_TIMEOUT, "Block validation timeout").into_response()
                    },
                    _ => {
                        (StatusCode::BAD_REQUEST, format!("Block validation error: {err}")).into_response()
                    }
                }
            },
            PreconfApiError::AlreadyProcessingNewerPayload => {
                (StatusCode::BAD_REQUEST, "Already processing newer payload").into_response()
            },
            PreconfApiError::AuctioneerError(err) => {
                (StatusCode::INTERNAL_SERVER_ERROR, format!("Auctioneer error: {err}")).into_response()
            },
            PreconfApiError::FeeRecipientMismatch { got, expected } => {
                (StatusCode::BAD_REQUEST, format!("Fee recipient mismatch. got: {got:?}, expected: {expected:?}")).into_response()
            },
            PreconfApiError::SlotMismatch { got, expected } => {
                (StatusCode::BAD_REQUEST, format!("Slot mismatch. got: {got}, expected: {expected}")).into_response()
            },
            PreconfApiError::MissingWithdrawls => {
                (StatusCode::BAD_REQUEST, "missing withdrawals").into_response()
            },
            PreconfApiError::InvalidWithdrawlsRoot => {
                (StatusCode::BAD_REQUEST, "invalid withdrawals root").into_response()
            },
            PreconfApiError::MissingWithdrawlsRoot => {
                (StatusCode::BAD_REQUEST, "missing withdrawals root").into_response()
            },
            PreconfApiError::WithdrawalsRootMismatch { got, expected } => {
                (StatusCode::BAD_REQUEST, format!("Withdrawals root mismatch. got: {got:?}, expected: {expected:?}")).into_response()
            },
            PreconfApiError::PayloadAttributesNotYetKnown => {
                (StatusCode::BAD_REQUEST, "payload attributes not yet known").into_response()
            },
            PreconfApiError::PrevRandaoMismatch { got, expected } => {
                (StatusCode::BAD_REQUEST, format!("Prev randao mismatch. got: {got:?}, expected: {expected:?}")).into_response()
            },
            PreconfApiError::DuplicateBlockHash { block_hash } => {
                (StatusCode::BAD_REQUEST, format!("block already received: {block_hash:?}")).into_response()
            },
            PreconfApiError::NotEnoughOptimisticCollateral {
                builder_pub_key,
                collateral,
                collateral_required,
                is_optimistic,
             } => {
                (StatusCode::BAD_REQUEST, format!(
                    "not enough optimistic collateral. builder_pub_key: {builder_pub_key:?}. 
                    collateral: {collateral:?}, collateral required: {collateral_required:?}. is_optimistic: {is_optimistic}"
                )).into_response()
            },
            PreconfApiError::BuilderNotOptimistic { builder_pub_key } => {
                (StatusCode::BAD_REQUEST, format!("builder is not optimistic. builder_pub_key: {builder_pub_key:?}")).into_response()
            },
            PreconfApiError::BuilderNotInProposersTrustedList { proposer_trusted_builders } => {
                (StatusCode::BAD_REQUEST, format!("builder not in proposer's trusted list: {proposer_trusted_builders:?}")).into_response()
            },
            PreconfApiError::V2SubmissionsInvalidIfProposerRequiresRegionalFiltering => {
                (StatusCode::BAD_REQUEST, "V2 submissions invalid if proposer requires regional filtering").into_response()
            }
        }
    }
}
