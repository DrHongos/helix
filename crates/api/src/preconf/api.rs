// Modified from builder/api.rs
// Intended to serve preconfirmation functions required
// TODO: 
// - create housekeep & gossip channel for this API
// - clean up dependencies
// - fix error types (used placeholders)
use std::{
    collections::HashMap, io::Read, sync::Arc
};

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    response::IntoResponse,
    Extension,
};
use ethereum_consensus::{
    ssz::{self, prelude::*},
};
use reth_primitives::{
    Bytes,
    PooledTransactionsElement, 
};
use futures::StreamExt;
use tokio::sync::{
    mpsc::{self, error::SendError, Sender},
    RwLock,
};
use tracing::{debug, error, info, warn};
//use uuid::Uuid;

use helix_common::{
    api::{
        builder_api::BuilderGetValidatorsResponseEntry,
        //proposer_api::ValidatorRegistrationInfo,
    },
    bid_submission::BidSubmission,
    chain_info::ChainInfo,
    signing::RelaySigningContext,
    simulator::BlockSimError,
    versioned_payload::PayloadAndBlobs,
    BuilderInfo, GossipedHeaderTrace, GossipedPayloadTrace, HeaderSubmissionTrace,
    SignedBuilderBid, SubmissionTrace,
};
use helix_datastore::Auctioneer;
use helix_housekeeper::{ChainUpdate, PayloadAttributesUpdate, SlotUpdate};

use crate::
  gossiper::{
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

        let api = Self {
            auctioneer,
 //           db,
            chain_info,
            gossiper,
//            signing_context,

            curr_slot_info: Arc::new(RwLock::new((0, None))),
            proposer_duties_response: Arc::new(RwLock::new(None)),
            payload_attributes: Arc::new(RwLock::new(HashMap::new())),

        };
// TODO need to implement both this protocols
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
            event = "submit_preconf_bundle",
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
        
        let transactions = req_v
            .into_iter()
            .map(recover_raw_transaction)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|tx| tx)
            .collect::<Vec<_>>();
        
        info!(
            event = "----------- received transactions ---------"
        );

        // We need to implement a gossip channel to update locally curr_slot_into (in housekeep)
        let (head_slot, next_duty) = api.curr_slot_info.read().await.clone();
        // store them in memory (some special place in auctioneer)
        api.auctioneer.save_bundle_beta_space_txs(
            head_slot,
            transactions,
        ).await?;

        Ok(StatusCode::OK)
    }
    
    // returns the bundle stored in a slot
    // TODO: return in response endpoint!
    pub async fn get_bundle_for_slot(
        Extension(api): Extension<Arc<PreconfApi<A, G>>>,
        req: Request<Body>,
    ) -> Result<StatusCode, PreconfApiError> {
        
        let body = req.into_body();
        let body_bytes = to_bytes(body, MAX_BLINDED_BLOCK_LENGTH).await?;
        match std::str::from_utf8(&body_bytes) {
            Ok(body_str) => {
                match body_str.parse::<u64>() {
                    Ok(slot) => {
                        match api.auctioneer.get_bundle_beta_space_txs(slot).await {
                            Ok(resp) => print!("Stored bundle: {:#?}", resp),
                            Err(err) => {
                                error!(error = %err, "failed to get beta space bundle");
                                return Err(PreconfApiError::PayloadAttributesNotYetKnown)
                            }
                        }
                    },
                    Err(err) => {
                        error!(error = %err, "failed to get slot number");
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


/// Recovers a [`PooledTransactionsElementEcRecovered`] from an enveloped encoded byte stream.
///
/// See [`PooledTransactionsElement::decode_enveloped`]
pub fn recover_raw_transaction(data: Bytes) -> Result<PooledTransactionsElement, PreconfApiError> {
    if data.is_empty() {
        return Err(PreconfApiError::PayloadAttributesNotYetKnown)
    }

    let transaction = PooledTransactionsElement::decode_enveloped(
        data.into(), 
    )
        .map_err(|_| PreconfApiError::FailedToDecodeHeaderSubmission)?;
    Ok(transaction)
}

// TODO: 
// create struct for the payload deserialization
// payloads limit
pub async fn deserialize_get_payload_vec_bytes(
    req: Request<Body>,
) -> Result<Vec<Bytes>, PreconfApiError> {
    let body = req.into_body();
    let body_bytes = to_bytes(body, MAX_BLINDED_BLOCK_LENGTH).await?;
    Ok(serde_json::from_slice(&body_bytes)?)
}

