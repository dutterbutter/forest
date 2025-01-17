// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

mod auth_api;
mod chain_api;
mod common_api;
mod gas_api;
mod mpool_api;
mod state_api;
mod sync_api;
mod wallet_api;

use crate::{common_api::version, state_api::*};
use async_log::span;
use async_std::net::{TcpListener, TcpStream};
use async_std::sync::{Arc, RwLock, Sender};
use async_std::task::{self, JoinHandle};
use async_tungstenite::{
    tungstenite::handshake::server::Request, tungstenite::Message, WebSocketStream,
};
use auth::{has_perms, Error as AuthError, JWT_IDENTIFIER, WRITE_ACCESS};
use blockstore::BlockStore;
use chain::{headchange_json::HeadChangeJson, EventsPayload};
use chain_sync::{BadBlockCache, SyncState};
use flo_stream::{MessagePublisher, Publisher, Subscriber};
use forest_libp2p::NetworkMessage;
use futures::future;
use futures::sink::SinkExt;
use futures::stream::{SplitSink, StreamExt};
use futures::TryFutureExt;
use jsonrpc_v2::{
    Data, Error, Id, MapRouter, RequestBuilder, RequestObject, ResponseObject, ResponseObjects,
    Server, V2,
};
use log::{debug, error, info, warn};
use message_pool::{MessagePool, MpoolRpcProvider};
use serde::Serialize;
use state_manager::StateManager;
use utils::get_home_dir;
use wallet::KeyStore;
use wallet::PersistentKeyStore;

type WsSink = SplitSink<WebSocketStream<TcpStream>, async_tungstenite::tungstenite::Message>;

const CHAIN_NOTIFY_METHOD_NAME: &str = "Filecoin.ChainNotify";
#[derive(Serialize)]
struct StreamingData<'a> {
    json_rpc: &'a str,
    method: &'a str,
    params: (usize, Vec<HeadChangeJson<'a>>),
}

/// This is where you store persistant data, or at least access to stateful data.
pub struct RpcState<DB, KS>
where
    DB: BlockStore + Send + Sync + 'static,
    KS: KeyStore + Send + Sync + 'static,
{
    pub state_manager: Arc<StateManager<DB>>,
    pub keystore: Arc<RwLock<KS>>,
    pub events_pubsub: Arc<RwLock<Publisher<EventsPayload>>>,
    pub mpool: Arc<MessagePool<MpoolRpcProvider<DB>>>,
    pub bad_blocks: Arc<BadBlockCache>,
    pub sync_state: Arc<RwLock<Vec<Arc<RwLock<SyncState>>>>>,
    pub network_send: Sender<NetworkMessage>,
    pub network_name: String,
}

pub async fn start_rpc<DB, KS>(state: RpcState<DB, KS>, rpc_endpoint: &str)
where
    DB: BlockStore + Send + Sync + 'static,
    KS: KeyStore + Send + Sync + 'static,
{
    use auth_api::*;
    use chain_api::*;
    use gas_api::*;
    use mpool_api::*;
    use sync_api::*;
    use wallet_api::*;
    let events_pubsub = state.events_pubsub.clone();
    let rpc = Server::new()
        .with_data(Data::new(state))
        // Auth API
        .with_method("Filecoin.AuthNew", auth_new::<DB, KS>, false)
        .with_method("Filecoin.AuthVerify", auth_verify::<DB, KS>, false)
        // Chain API
        .with_method(
            "Filecoin.ChainGetMessage",
            chain_api::chain_get_message::<DB, KS>,
            false,
        )
        .with_method("Filecoin.ChainGetObj", chain_read_obj::<DB, KS>, false)
        .with_method("Filecoin.ChainHasObj", chain_has_obj::<DB, KS>, false)
        .with_method(
            "Filecoin.ChainGetBlockMessages",
            chain_block_messages::<DB, KS>,
            false,
        )
        .with_method(
            "Filecoin.ChainGetTipsetByHeight",
            chain_get_tipset_by_height::<DB, KS>,
            false,
        )
        .with_method(
            "Filecoin.ChainGetGenesis",
            chain_get_genesis::<DB, KS>,
            false,
        )
        .with_method(
            "Filecoin.ChainTipsetWeight",
            chain_tipset_weight::<DB, KS>,
            false,
        )
        .with_method("Filecoin.ChainGetTipset", chain_get_tipset::<DB, KS>, false)
        .with_method(
            "Filecoin.GetRandomness",
            chain_get_randomness::<DB, KS>,
            false,
        )
        .with_method(
            "Filecoin.ChainGetBlock",
            chain_api::chain_get_block::<DB, KS>,
            false,
        )
        .with_method(CHAIN_NOTIFY_METHOD_NAME, chain_notify::<DB, KS>, true)
        .with_method("Filecoin.ChainHead", chain_head::<DB, KS>, false)
        // Message Pool API
        .with_method(
            "Filecoin.MpoolEstimateGasPrice",
            estimate_gas_premium::<DB, KS>,
            false,
        )
        .with_method(
            "Filecoin.MpoolGetNonce",
            mpool_get_sequence::<DB, KS>,
            false,
        )
        .with_method("Filecoin.MpoolPending", mpool_pending::<DB, KS>, false)
        .with_method("Filecoin.MpoolPush", mpool_push::<DB, KS>, false)
        .with_method(
            "Filecoin.MpoolPushMessage",
            mpool_push_message::<DB, KS>,
            false,
        )
        // Sync API
        .with_method("Filecoin.SyncCheckBad", sync_check_bad::<DB, KS>, false)
        .with_method("Filecoin.SyncMarkBad", sync_mark_bad::<DB, KS>, false)
        .with_method("Filecoin.SyncState", sync_state::<DB, KS>, false)
        .with_method(
            "Filecoin.SyncSubmitBlock",
            sync_submit_block::<DB, KS>,
            false,
        )
        // Wallet API
        .with_method("Filecoin.WalletBalance", wallet_balance::<DB, KS>, false)
        .with_method(
            "Filecoin.WalletDefaultAddress",
            wallet_default_address::<DB, KS>,
            false,
        )
        .with_method("Filecoin.WalletExport", wallet_export::<DB, KS>, false)
        .with_method("Filecoin.WalletHas", wallet_has::<DB, KS>, false)
        .with_method("Filecoin.WalletImport", wallet_import::<DB, KS>, false)
        .with_method("Filecoin.WalletList", wallet_list::<DB, KS>, false)
        .with_method("Filecoin.WalletNew", wallet_new::<DB, KS>, false)
        .with_method(
            "Filecoin.WalletSetDefault",
            wallet_set_default::<DB, KS>,
            false,
        )
        .with_method("Filecoin.WalletSign", wallet_sign::<DB, KS>, false)
        .with_method(
            "Filecoin.WalletSignMessage",
            wallet_sign_message::<DB, KS>,
            false,
        )
        .with_method("Filecoin.WalletVerify", wallet_verify::<DB, KS>, false)
        // State API
        .with_method(
            "Filecoin.StateMinerSector",
            state_miner_sector::<DB, KS>,
            false,
        )
        .with_method("Filecoin.StateCall", state_call::<DB, KS>, false)
        .with_method(
            "Filecoin.StateMinerDeadlines",
            state_miner_deadlines::<DB, KS>,
            false,
        )
        .with_method(
            "Filecoin.StateSectorPrecommitInfo",
            state_sector_precommit_info::<DB, KS>,
            false,
        )
        .with_method(
            "Filecoin.StateSectorInfo",
            state_sector_info::<DB, KS>,
            false,
        )
        .with_method(
            "Filecoin.StateMinerProvingDeadline",
            state_miner_proving_deadline::<DB, KS>,
            false,
        )
        .with_method("Filecoin.StateMinerInfo", state_miner_info::<DB, KS>, false)
        .with_method(
            "Filecoin.StateMinerFaults",
            state_miner_faults::<DB, KS>,
            false,
        )
        .with_method(
            "Filecoin.StateAllMinerFaults",
            state_all_miner_faults::<DB, KS>,
            false,
        )
        .with_method(
            "Filecoin.StateMinerRecoveries",
            state_miner_recoveries::<DB, KS>,
            false,
        )
        .with_method("Filecoin.StateReplay", state_replay::<DB, KS>, false)
        .with_method("Filecoin.StateGetActor", state_get_actor::<DB, KS>, false)
        .with_method(
            "Filecoin.StateAccountKey",
            state_account_key::<DB, KS>,
            false,
        )
        .with_method("Filecoin.StateLookupId", state_lookup_id::<DB, KS>, false)
        .with_method(
            "Filecoin.StateMartketBalance",
            state_market_balance::<DB, KS>,
            false,
        )
        .with_method(
            "Filecoin.StateGetReceipt",
            state_get_receipt::<DB, KS>,
            false,
        )
        .with_method("Filecoin.StateWaitMsg", state_wait_msg::<DB, KS>, false)
        .with_method("Filecoin.NetworkName", state_network_name::<DB, KS>, false)
        // Gas API
        .with_method(
            "Filecoin.GasEstimateGasLimit",
            gas_estimate_gas_limit::<DB, KS>,
            false,
        )
        .with_method(
            "Filecoin.GasEstimateGasPremium",
            gas_estimate_gas_premium::<DB, KS>,
            false,
        )
        .with_method(
            "Filecoin.GasEstimateFeeCap",
            gas_estimate_fee_cap::<DB, KS>,
            false,
        )
        // Common
        .with_method("Filecoin.Version", version, false)
        .finish_unwrapped();

    let try_socket = TcpListener::bind(rpc_endpoint).await;
    let listener = try_socket.expect("Failed to bind to addr");
    let rpc_state = Arc::new(rpc);

    info!("waiting for web socket connections");
    while let Ok((stream, addr)) = listener.accept().await {
        let subscriber = events_pubsub.write().await.subscribe();
        task::spawn(handle_connection_and_log(
            rpc_state.clone(),
            stream,
            addr,
            events_pubsub.clone(),
            subscriber,
        ));
    }

    info!("Stopped accepting websocket connections");
}

async fn handle_connection_and_log(
    state: Arc<Server<MapRouter>>,
    tcp_stream: TcpStream,
    addr: std::net::SocketAddr,
    events_out: Arc<RwLock<Publisher<EventsPayload>>>,
    events_in: Subscriber<EventsPayload>,
) {
    span!("handle_connection_and_log", {
        let mut authorization_header: Option<String> = None;
        if let Ok(ws_stream) =
            async_tungstenite::accept_hdr_async(tcp_stream, |request: &Request, response| {
                if let Some(authorization) = request.headers().get("Authorization") {
                    // not all methods require authorization
                    authorization_header = authorization
                        .to_str()
                        .map(|s| Some(s.to_string()))
                        .unwrap_or_default();
                }
                Ok(response)
            })
            .await
        {
            debug!("accepted websocket connection at {:}", addr);
            let (ws_sender, mut ws_receiver) = ws_stream.split();
            let ws_sender = Arc::new(RwLock::new(ws_sender));
            let mut chain_notify_count: usize = 0;
            while let Some(message_result) = ws_receiver.next().await {
                match message_result {
                    Ok(message) => {
                        let request_text = message.into_text().unwrap();
                        match serde_json::from_str(&request_text)
                            as Result<RequestObject, serde_json::Error>
                        {
                            Ok(call) => {
                                // hacky but due to the limitations of jsonrpc_v2 impl
                                // if this expands, better to implement some sort of middleware

                                let call = if &*call.method == CHAIN_NOTIFY_METHOD_NAME {
                                    chain_notify_count += 1;
                                    RequestBuilder::default()
                                        .with_id(call.id.unwrap_or_default().unwrap_or_default())
                                        .with_params(chain_notify_count)
                                        .with_method(CHAIN_NOTIFY_METHOD_NAME)
                                        .finish()
                                } else {
                                    call
                                };
                                let response = handle_rpc(&state, call, &authorization_header)
                                    .await
                                    .unwrap_or_else(|e| {
                                        ResponseObjects::One(ResponseObject::Error {
                                            jsonrpc: V2,
                                            error: Error::Full {
                                                code: 1,
                                                message: e.message(),
                                                data: None,
                                            },
                                            id: Id::Null,
                                        })
                                    });
                                let error_send = ws_sender.clone();

                                // initiate response and streaming if applicable
                                let join_handle = streaming_payload(
                                    ws_sender.clone(),
                                    response,
                                    chain_notify_count,
                                    events_out.clone(),
                                    events_in.clone(),
                                )
                                .map_err(|e| async move {
                                    send_error(
                                        3,
                                        &error_send,
                                        format!(
                                            "channel id {:}, error {:?}",
                                            chain_notify_count,
                                            e.message()
                                        ),
                                    )
                                    .await
                                    .unwrap_or_else(|e| {
                                        error!("error {:?} on socket {:?}", e.message(), addr)
                                    });
                                })
                                .await
                                .unwrap_or_else(|_| {
                                    error!("error sending on socket {:?}", addr);
                                    None
                                });

                                // wait for join handle to complete if there is error and send it over the network and cancel streaming
                                let error_join_send = ws_sender.clone();
                                let handle_events_out = events_out.clone();
                                task::spawn(async move {
                                    if let Some(handle) = join_handle {
                                        handle
                                            .map_err(|e| async move {
                                                send_error(
                                                    3,
                                                    &error_join_send,
                                                    format!(
                                                        "channel id {:}, error {:?}",
                                                        chain_notify_count,
                                                        e.message()
                                                    ),
                                                )
                                                .await
                                                .unwrap_or_else(|e| {
                                                    error!(
                                                        "error {:?} on socket {:?}",
                                                        e.message(),
                                                        addr
                                                    )
                                                });
                                            })
                                            .await
                                            .unwrap_or_else(|_| {
                                                error!("error sending on socket {:?}", addr)
                                            });

                                        handle_events_out
                                            .write()
                                            .await
                                            .publish(EventsPayload::TaskCancel(
                                                chain_notify_count,
                                                (),
                                            ))
                                            .await;
                                    } else {
                                        handle_events_out
                                            .write()
                                            .await
                                            .publish(EventsPayload::TaskCancel(
                                                chain_notify_count,
                                                (),
                                            ))
                                            .await
                                    }
                                });
                            }
                            Err(e) => send_error(1, &ws_sender, e.to_string())
                                .await
                                .unwrap_or_else(|e| {
                                    error!("error {:?} on socket {:?}", e.message(), addr)
                                }),
                        }
                    }
                    Err(e) => send_error(2, &ws_sender, e.to_string())
                        .await
                        .unwrap_or_else(|e| error!("error {:?} on socket {:?}", e.message(), addr)),
                };
            }
        } else {
            warn!("web socket connection failed at {:}", addr)
        }
    })
}

async fn handle_rpc(
    state: &Arc<Server<MapRouter>>,
    call: RequestObject,
    authorization_header: &Option<String>,
) -> Result<ResponseObjects, Error> {
    if WRITE_ACCESS.contains(&&*call.method) {
        if let Some(header) = authorization_header {
            let keystore = PersistentKeyStore::new(get_home_dir() + "/.forest")?;
            let ki = keystore
                .get(JWT_IDENTIFIER)
                .map_err(|_| AuthError::Other("No JWT private key found".to_owned()))?;
            let key = ki.private_key();
            let perms = has_perms(header.to_string(), "write", key);
            if perms.is_err() {
                return Err(perms.unwrap_err());
            }
        } else {
            return Ok(ResponseObjects::One(ResponseObject::Error {
                jsonrpc: V2,
                error: Error::Full {
                    code: 200,
                    message: AuthError::NoAuthHeader.to_string(),
                    data: None,
                },
                id: Id::Null,
            }));
        }
    };

    Ok(state.handle(call).await)
}

async fn send_error(code: i64, ws_sender: &RwLock<WsSink>, message: String) -> Result<(), Error> {
    let response = ResponseObjects::One(ResponseObject::Error {
        jsonrpc: V2,
        error: Error::Full {
            code,
            message,
            data: None,
        },
        id: Id::Null,
    });
    let response_text = serde_json::to_string(&response)?;
    ws_sender
        .write()
        .await
        .send(Message::text(response_text))
        .await?;
    Ok(())
}
async fn streaming_payload(
    ws_sender: Arc<RwLock<WsSink>>,
    response_object: ResponseObjects,
    streaming_count: usize,
    events_out: Arc<RwLock<Publisher<EventsPayload>>>,
    events_in: Subscriber<EventsPayload>,
) -> Result<Option<JoinHandle<Result<(), Error>>>, Error> {
    let response_text = serde_json::to_string(&response_object)?;
    ws_sender
        .write()
        .await
        .send(Message::text(response_text))
        .await?;
    if let ResponseObjects::One(ResponseObject::Result {
        jsonrpc: _,
        result: _,
        id: _,
        streaming,
    }) = response_object
    {
        if streaming {
            let handle = task::spawn(async move {
                let mut filter_on_channel_id = events_in.filter(|s| {
                    future::ready(
                        s.sub_head_changes()
                            .map(|s| s.0 == streaming_count)
                            .unwrap_or_default(),
                    )
                });
                while let Some(event) = filter_on_channel_id.next().await {
                    if let EventsPayload::SubHeadChanges(ref index_to_head_change) = event {
                        if streaming_count == index_to_head_change.0 {
                            let head_change = (&index_to_head_change.1).into();
                            let data = StreamingData {
                                json_rpc: "2.0",
                                method: "xrpc.ch.val",
                                params: (streaming_count, vec![head_change]),
                            };
                            let response_text = serde_json::to_string(&data)?;
                            ws_sender
                                .write()
                                .await
                                .send(Message::text(response_text))
                                .await?;
                        }
                    }
                }

                Ok::<(), Error>(())
            });

            Ok(Some(handle))
        } else {
            Ok(None)
        }
    } else {
        events_out
            .write()
            .await
            .publish(EventsPayload::TaskCancel(streaming_count, ()))
            .await;
        Ok(None)
    }
}
