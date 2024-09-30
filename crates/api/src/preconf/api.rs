// Modified from builder/api.rs
// Intended to serve preconfirmation functions required
// TODO: prepare /beta_transactions_request endpoint
// to store a bundle of transactions to be included in a slot block
use std::{
    collections::HashMap, io::Read, ops::Deref, sync::Arc, time::{Duration, SystemTime, UNIX_EPOCH}
};

use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade, Message},
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Extension,
};
use ethereum_consensus::{
    configs::mainnet::{CAPELLA_FORK_EPOCH, SECONDS_PER_SLOT},
    phase0::mainnet::SLOTS_PER_EPOCH,
    primitives::{BlsPublicKey, Bytes32, Hash32},
    ssz::{self, prelude::*},
};
use flate2::read::GzDecoder;
use futures::StreamExt;
use hyper::HeaderMap;
use tokio::{
    sync::{
        mpsc::{self, error::SendError, Receiver, Sender},
        RwLock,
    },
    time::{self, Instant},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use helix_common::{
    api::{
        builder_api::{BuilderGetValidatorsResponse, BuilderGetValidatorsResponseEntry},
        proposer_api::ValidatorRegistrationInfo,
    },
    bid_submission::{
        v2::header_submission::{
            SignedHeaderSubmission, SignedHeaderSubmissionCapella, SignedHeaderSubmissionDeneb,
        },
        BidSubmission, BidTrace, SignedBidSubmission,
    },
    chain_info::ChainInfo,
    signing::RelaySigningContext,
    simulator::BlockSimError,
    versioned_payload::PayloadAndBlobs,
    BuilderInfo, GossipedHeaderTrace, GossipedPayloadTrace, HeaderSubmissionTrace,
    SignedBuilderBid, SubmissionTrace,
};
use helix_database::DatabaseService;
use helix_datastore::{types::SaveBidAndUpdateTopBidResponse, Auctioneer};
use helix_housekeeper::{ChainUpdate, PayloadAttributesUpdate, SlotUpdate};
use helix_utils::{calculate_withdrawals_root, get_payload_attributes_key, has_reached_fork, try_decode_into};

use crate::{builder::{
    error::BuilderApiError, traits::BlockSimulator, BlockSimRequest, DbInfo, OptimisticVersion,
}, gossiper::{
    traits::GossipClientTrait,
    types::{BroadcastHeaderParams, BroadcastPayloadParams, GossipedMessage},
}};

pub(crate) const MAX_PAYLOAD_LENGTH: usize = 1024 * 1024 * 10;

#[derive(Clone)]
pub struct PreconfApi<A, DB>
where
    A: Auctioneer + 'static,
    DB: DatabaseService + 'static,
{
    auctioneer: Arc<A>,
    db: Arc<DB>,
    chain_info: Arc<ChainInfo>,
//    simulator: S,
//    gossiper: Arc<G>,
//    signing_context: Arc<RelaySigningContext>,

    db_sender: Sender<DbInfo>,

    /// Information about the current head slot and next proposer duty
    curr_slot_info: Arc<RwLock<(u64, Option<BuilderGetValidatorsResponseEntry>)>>,

    proposer_duties_response: Arc<RwLock<Option<Vec<u8>>>>,
    payload_attributes: Arc<RwLock<HashMap<String, PayloadAttributesUpdate>>>,
}

impl<A, DB> PreconfApi<A, DB>
where
    A: Auctioneer + 'static,
    DB: DatabaseService + 'static,
{
    pub fn new(
        auctioneer: Arc<A>,
        db: Arc<DB>,
        chain_info: Arc<ChainInfo>,
        //slot_update_subscription: Sender<Sender<ChainUpdate>>,
        //gossip_receiver: Receiver<GossipedMessage>,
    ) -> Self {
        let (db_sender, db_receiver) = mpsc::channel::<DbInfo>(10_000);

        // Spin up db processing task
        let db_clone = db.clone();
        tokio::spawn(async move {
            process_db_additions(db_clone, db_receiver).await;
        });

        let api = Self {
            auctioneer,
            db,
            chain_info,
//            simulator,
//            gossiper,
//            signing_context,

            db_sender,

            curr_slot_info: Arc::new(RwLock::new((0, None))),
            proposer_duties_response: Arc::new(RwLock::new(None)),
            payload_attributes: Arc::new(RwLock::new(HashMap::new())),
        };

        // Spin up gossip processing task
//        let api_clone = api.clone();
//        tokio::spawn(async move {
//            api_clone.process_gossiped_info(gossip_receiver).await;
//        });

        // Spin up the housekeep task.
        // This keeps the curr slot info variables up to date.
//        let api_clone = api.clone();
//        tokio::spawn(async move {
//            if let Err(err) = api_clone.housekeep(slot_update_subscription.clone()).await {
//                error!(
//                    error = %err,
//                    "BuilderApi. housekeep task encountered an error",
//                );
//            }
//        });

        api
    }


    pub async fn submit_preconf_bundle(
        Extension(api): Extension<Arc<PreconfApi<A, DB>>>,
        req: Request<Body>,
    ) -> Result<StatusCode, PreconfApiError> {

        info!(
            request_id = %request_id,
            event = "submit_preconf_bundle",
            head_slot = head_slot,
            timestamp_request_start = trace.receive,
        );



        Ok(StatusCode::OK)
    }

}
