use ibc_relayer_types::events::IbcEvent;
use std::{thread, time::Duration};
use tracing::{error, info, warn};

use crate::chain::handle::ChainHandle;
use crate::chain::requests::{PageRequest, QueryClientStatesRequest};
use crate::chain::tracking::{NonCosmosTrackingId, TrackedMsgs, TrackingId};
use crate::client_state::IdentifiedAnyClientState;
use crate::config::ChainConfig;
use crate::error::ErrorDetail::LightClientVerification;
use crate::event::monitor::EventBatch;
use tendermint_light_client::errors::ErrorDetail::MissingLastBlockId;

const MAX_HEADERS_IN_BATCH: u64 = 32;
const MAX_RETRY_SLEEP_INTERVAL: Duration = Duration::from_secs(12);
const MAX_RETRY_NUMBER: u8 = 5;
pub fn handle_event_batch<ChainA: ChainHandle, ChainB: ChainHandle>(
    eth_chain: &ChainA,
    ckb_chain: &ChainB,
    event_batch: &EventBatch,
) {
    let dst_chain = ckb_chain;
    let src_chain = eth_chain;
    if !matches!(src_chain.config().unwrap(), ChainConfig::Eth(_))
        || !matches!(dst_chain.config().unwrap(), ChainConfig::Ckb(_))
    {
        error!("ignore header relay while src chain is not eth or dst chain is not ckb");
        error!("src_chain: {:?}", src_chain);
        error!("dst_chain: {:?}", dst_chain);
        return;
    }

    if event_batch.events.is_empty() {
        warn!("CAUTION: start to relay EMPTY headers");
        return;
    }

    // assemble client states which are transformed from fianlity headers
    let mut start_slot = 0;
    let end_slot = event_batch.height.revision_height();
    info!("start to relay headers up to {}", end_slot);
    let any_client_states = event_batch
        .events
        .iter()
        .filter_map(|event| {
            if let IbcEvent::NewBlock(new_block) = event.event {
                if start_slot == 0 {
                    start_slot = new_block.height.revision_height();
                }
                let client_state = {
                    let client_state = src_chain.build_client_state(
                        new_block.height,
                        crate::chain::client::ClientSettings::Other,
                    );
                    match client_state {
                        Ok(value) => value,
                        Err(err) => {
                            error!("src_chain.build_client_state: {}", err);
                            return None;
                        }
                    }
                };
                return Some(client_state.into());
            }
            None
        })
        .collect();

    let tracked_msgs = TrackedMsgs {
        msgs: any_client_states,
        tracking_id: TrackingId::Static(NonCosmosTrackingId::ETH_UPDATE_CLIENT),
    };

    // try sending header
    let result = dst_chain.send_messages_and_wait_commit(tracked_msgs);
    if result.is_ok() {
        info!("finish relay headers from {} to {}", start_slot, end_slot);
        return;
    }

    // returned err indicates headers falling behind
    let error = result.unwrap_err();
    let mut start_height = match error.detail() {
        LightClientVerification(e) => match &e.source {
            MissingLastBlockId(height) => {
                warn!(
                    "header {} is beyond onchain or native tip header {}, start to chase",
                    start_slot, height.height
                );
                height.height.into()
            }
            _ => {
                error!("receive unexpected error: {:?}", error);
                return;
            }
        },
        _ => {
            error!("receive unexpected error: {:?}", error);
            return;
        }
    };

    // start to chase lost headers
    let target_height = end_slot;
    let mut retry_number = 0;
    while start_height < target_height {
        if retry_number > 0 {
            info!(
                "{} time retry from height {} to height {}",
                retry_number, start_height, target_height
            );
        } else {
            info!(
                "continue to chase headers from {} to {}",
                start_height, target_height
            );
        }
        let limit = std::cmp::min(MAX_HEADERS_IN_BATCH, target_height - start_height + 1);
        let request = QueryClientStatesRequest {
            pagination: Some(PageRequest {
                offset: start_height,
                limit,
                ..Default::default()
            }),
        };
        let client_states = {
            let client_states = src_chain.query_clients(request);
            match client_states {
                Ok(value) => value,
                Err(err) => {
                    error!("src_chain.query_clients: {}", err);
                    return;
                }
            }
        };
        let clients_len = client_states.len() as u64;
        let end_height = start_height + clients_len - 1;
        if clients_len < limit {
            warn!(
                "can't find enough headers to relay, expect {} but only fetch {}",
                limit, clients_len
            );
        }
        info!(
            "send chased headers from {} to {}",
            start_height, end_height
        );
        match send_messages(dst_chain, client_states) {
            Ok(_) => {
                info!(
                    "headers from {} to {} are relayed to CKB",
                    start_height, end_height
                );
                retry_number = 0;
                start_height = end_height + 1;
            }
            Err(e) => {
                error!("encounter error and wait retry: {}", e);
                retry_number += 1;
                if let LightClientVerification(e) = e.detail() {
                    if let MissingLastBlockId(_) = e.source {
                        thread::sleep(MAX_RETRY_SLEEP_INTERVAL);
                    }
                }
            }
        }
        if retry_number >= MAX_RETRY_NUMBER {
            error!(
                "retry number {} exceeds maximum value {}, stop retry process.",
                retry_number, MAX_RETRY_NUMBER
            );
            return;
        }
    }
}

fn send_messages<Chain: ChainHandle>(
    chain: &Chain,
    client_states: Vec<IdentifiedAnyClientState>,
) -> Result<Vec<crate::event::IbcEventWithHeight>, crate::error::Error> {
    let tracked_msgs = TrackedMsgs {
        msgs: client_states
            .into_iter()
            .map(|s| s.client_state.into())
            .collect(),
        tracking_id: TrackingId::Static(NonCosmosTrackingId::ETH_UPDATE_CLIENT),
    };
    chain.send_messages_and_wait_commit(tracked_msgs)
}