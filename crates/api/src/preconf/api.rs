// Modified from builder/api.rs
// Intended to serve preconfirmation functions required
// TODO: prepare /beta_transactions_request endpoint
// to store a bundle of transactions to be included in a slot block
use std::{
    collections::HashMap, io::Read, sync::Arc, time::{Duration, SystemTime, UNIX_EPOCH}, sync::Mutex
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
    primitives::{BlsPublicKey,  Hash32},
    ssz::{self, prelude::*},
};
use reth_primitives::{
    Bytes,
    transaction::{TransactionSigned, TxType},
    PooledTransactionsElement, 
    PooledTransactionsElementEcRecovered,
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
        v2::header_submission::SignedHeaderSubmission,
        BidSubmission, BidTrace, SignedBidSubmission,
    },
    chain_info::ChainInfo,
    signing::RelaySigningContext,
    simulator::BlockSimError,
    versioned_payload::PayloadAndBlobs,
    BuilderInfo, GossipedHeaderTrace, GossipedPayloadTrace, HeaderSubmissionTrace,
    SignedBuilderBid, SubmissionTrace,
};
use helix_datastore::{types::SaveBidAndUpdateTopBidResponse, Auctioneer};
use helix_housekeeper::{ChainUpdate, PayloadAttributesUpdate, SlotUpdate};
use helix_utils::{get_payload_attributes_key, has_reached_fork, try_decode_into};

use crate::{builder::{
    error::BuilderApiError, traits::BlockSimulator, BlockSimRequest, DbInfo, OptimisticVersion,
}, gossiper::{
    traits::GossipClientTrait,
    types::{BroadcastHeaderParams, BroadcastPayloadParams, GossipedMessage},
},    
    preconf::error::PreconfApiError,
};

pub(crate) const MAX_PAYLOAD_LENGTH: usize = 1024 * 1024 * 10;
pub(crate) const MAX_BLINDED_BLOCK_LENGTH: usize = 1024 * 1024;// FIXME: rename and check

#[derive(Clone)]
pub struct PreconfApi<A, G>
where
    A: Auctioneer + 'static,
    G: GossipClientTrait + 'static,
{
    auctioneer: Arc<A>,
    gossiper: Arc<G>,
    chain_info: Arc<ChainInfo>,

    //db_sender: Sender<DbInfo>,

    /// Information about the current head slot and next proposer duty
    curr_slot_info: Arc<RwLock<(u64, Option<BuilderGetValidatorsResponseEntry>)>>,

    proposer_duties_response: Arc<RwLock<Option<Vec<u8>>>>,
    payload_attributes: Arc<RwLock<HashMap<String, PayloadAttributesUpdate>>>,

}

impl<A, G> PreconfApi<A, G>
where
    A: Auctioneer + 'static,
    G: GossipClientTrait + 'static,
{
    pub fn new(
        auctioneer: Arc<A>,
        chain_info: Arc<ChainInfo>,
        gossiper: Arc<G>,
        //slot_update_subscription: Sender<Sender<ChainUpdate>>,
//        gossip_receiver: Receiver<GossipedMessage>,
    ) -> Self {
        //let (db_sender, db_receiver) = mpsc::channel::<DbInfo>(10_000);

        // Spin up db processing task
//        let db_clone = db.clone();
//        tokio::spawn(async move {
//            process_db_additions(db_clone, db_receiver).await;
//        });

        let api = Self {
            auctioneer,
 //           db,
            chain_info,
//            simulator,
            gossiper,
//            signing_context,

 //           db_sender,

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

    pub async fn test() -> Result<StatusCode, PreconfApiError> {
    	info!(event = "------------- Testing preconf API -------------");
	Ok(StatusCode::OK)
    }

    pub async fn submit_preconf_bundle(
        Extension(api): Extension<Arc<PreconfApi<A, G>>>,
        req: Request<Body>,
    ) -> Result<StatusCode, PreconfApiError> {

        info!(
            //request_id = %request_id,
            event = "submit_preconf_bundle",
            //head_slot = head_slot,
            //timestamp_request_start = trace.receive,
        );

        // extract bundle of req body
        let req_v: Vec<Bytes> = match deserialize_get_payload_vec_bytes(req).await {
            Ok(vec_tx) => vec_tx,
            Err(err) => {
                warn!(
                    event = "decoding preconf bundle",
                    error = %err,
                    "failed to decode preconf bundle",
                );
                return Err(err);
            }
        };
        
        // and then, copying reth/rpc/rpc/eth/bundle (remember this is not in the rev of reth used
        // in this repository)
        // https://reth.rs/docs/src/reth_rpc/eth/bundle.rs.html#76
        let transactions = req_v
            .into_iter()
            .map(recover_raw_transaction)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|tx| tx)
            .collect::<Vec<_>>();
        // see how it manages filters and limits..

        info!(
            event = "----------- received transactions ---------"
        );
        print!("Received txs: {:#?} ", transactions);

        // simulate tx (and validate) ..or maybe not? (will have to do it in the re-bundle process)
        let (head_slot, next_duty) = api.curr_slot_info.read().await.clone();
        // store them in memory (some special place in auctioneer)
        print!("Stored in slot: {}", head_slot);
        api.auctioneer.save_bundle_beta_space_txs(
            head_slot, // FIXME : check if current or next
            transactions,
        ).await?;
        // go to helix method get_payload where it should be used

        Ok(StatusCode::OK)
    }
    
    // returns the bundle stored in a slot
    // TODO: return in response
    // endpoint!
    pub async fn get_bundle_for_slot(
        Extension(api): Extension<Arc<PreconfApi<A, G>>>,
        req: Request<Body>,
    ) -> Result<StatusCode, PreconfApiError> {
        
        let body = req.into_body();
        let body_bytes = to_bytes(body, MAX_BLINDED_BLOCK_LENGTH).await?;
        //print!("bytes rec: {:#?}", body_bytes);

        match std::str::from_utf8(&body_bytes) {
            Ok(body_str) => {
                print!("str rec: {}", body_str);
                match body_str.parse::<u64>() {
                    Ok(slot) => {
                        match api.auctioneer.get_bundle_beta_space_txs(slot).await {
                            Ok(resp) => print!("{:#?}", resp),
                            Err(err) => {
                                error!(error = %err, "failed to get beta space bundle");
                                return Err(PreconfApiError::PayloadAttributesNotYetKnown)
                            }
                        }
                    },
                    Err(err) => {
                        error!(error = %err, "failed to get beta space string");
                        return Err(PreconfApiError::PayloadAttributesNotYetKnown)
                    }
                }
            },
            Err(err) => {
                error!(error = %err, "failed to parse slot in payload");
                return Err(PreconfApiError::PayloadAttributesNotYetKnown)
            }
        }
        Ok(StatusCode::OK)
    }
}

// handle gossiped payloads
impl<A, G> PreconfApi<A, G> 
where
    A: Auctioneer + 'static,
    G: GossipClientTrait + 'static,
{

    pub async fn housekeep(
        &self,
        slot_update_subscription: Sender<Sender<ChainUpdate>>,
    ) -> Result<(), SendError<Sender<ChainUpdate>>> {
        let (tx, mut rx) = mpsc::channel(20);
        slot_update_subscription.send(tx).await?;

        while let Some(slot_update) = rx.recv().await {
            match slot_update {
                ChainUpdate::SlotUpdate(slot_update) => {
                    self.handle_new_slot(slot_update).await;
                }
                ChainUpdate::PayloadAttributesUpdate(payload_attributes) => {
                    //self.handle_new_payload_attributes(payload_attributes).await;
                }
            }
        }

        Ok(())
    }

    /// Handle a new slot update.
    /// Updates the next proposer duty and prepares the get_validators() response.
    async fn handle_new_slot(&self, slot_update: SlotUpdate) {
        let epoch = slot_update.slot / SLOTS_PER_EPOCH;
        debug!(
            epoch = epoch,
            slot_head = slot_update.slot,
            slot_start_next_epoch = (epoch + 1) * SLOTS_PER_EPOCH,
            next_proposer_duty = ?slot_update.next_duty,
            "updated head slot",
        );

        *self.curr_slot_info.write().await = (slot_update.slot, slot_update.next_duty);

//        if let Some(new_duties) = slot_update.new_duties {
//            let response: Vec<BuilderGetValidatorsResponse> =
//                new_duties.into_iter().map(|duty| duty.into()).collect();
//            match serde_json::to_vec(&response) {
//                Ok(duty_bytes) => *self.proposer_duties_response.write().await = Some(duty_bytes),
//                Err(err) => {
//                    error!(error = %err, "failed to serialize proposer duties to JSON");
//                    *self.proposer_duties_response.write().await = None;
//                }
//            }
//        }
    }


}


// taken from reth (not in revision used) modified result and error types
// TODO: fix errors (here just existent (and invalid) placeholders)
/// Recovers a [`PooledTransactionsElementEcRecovered`] from an enveloped encoded byte stream.
///
/// See [`PooledTransactionsElement::decode_enveloped`]
pub fn recover_raw_transaction(data: Bytes) -> Result<PooledTransactionsElement, PreconfApiError> {
    if data.is_empty() {
        return Err(PreconfApiError::PayloadAttributesNotYetKnown)
    }

    let transaction = PooledTransactionsElement::decode_enveloped(
        data.into(), // revision based (Bytes)
    // original: &mut data.as_ref()
    )
        .map_err(|_| PreconfApiError::FailedToDecodeHeaderSubmission)?;
    Ok(transaction)

//    transaction.try_into_ecrecovered().or(Err(PreconfApiError::ProposerDutyNotFound))
}

// TODO: 
// create struct for the payload deserialization
// payloads limit
// remove prints
pub async fn deserialize_get_payload_vec_bytes(
    req: Request<Body>,
) -> Result<Vec<Bytes>, PreconfApiError> {
    let body = req.into_body();
    let body_bytes = to_bytes(body, MAX_BLINDED_BLOCK_LENGTH).await?;
    print!("{:?}", body_bytes);
    Ok(serde_json::from_slice(&body_bytes)?)
}

