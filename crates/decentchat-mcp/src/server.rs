//! MCP server implementation for DecentChat.

use std::net::SocketAddr;
use std::sync::Arc;

use decentchat_core::{ChatEvent, GroupId, Message};
use decentchat_protocol::{
    BootstrapPeer, ConnectionTicket, GroupSession, Identity, QuicTransport, QuicTransportConfig,
    SessionConfig, SessionEventReceiver, Transport,
};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    AnnotateAble, ListResourcesResult, RawResource, ReadResourceRequestParams,
    ReadResourceResult, ResourceContents, ServerCapabilities, ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::{tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tracing::{debug, info, warn};

use crate::resources::{uri, MessagesResource, StatusResource, UsersResource};
use crate::tools::{
    GetNewMessagesResult, GetTicketResult, JoinRoomResult, LeaveRoomResult, MessageInfo,
    SendMessageResult, SetNicknameResult, StatusInfo, UserInfo,
};

/// Maximum number of messages to keep for polling.
const MAX_MESSAGE_HISTORY: usize = 100;

/// Commands sent to the session task.
enum SessionCommand {
    SendMessage {
        content: String,
        reply: oneshot::Sender<Result<Message, String>>,
    },
    SetUsername {
        name: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Leave {
        reply: oneshot::Sender<Result<(), String>>,
    },
    GetUsers {
        reply: oneshot::Sender<Vec<UserInfo>>,
    },
}

/// Request to join a room (sent to a spawned task).
struct JoinRequest {
    room_name: String,
    bootstrap: Vec<BootstrapPeer>,
    nickname: Option<String>,
    reply: oneshot::Sender<Result<JoinResult, String>>,
}

/// Result of a successful join.
struct JoinResult {
    ticket: String,
    cmd_tx: mpsc::Sender<SessionCommand>,
    initial_status: SessionStatus,
}

/// Status information about the session.
#[derive(Clone)]
struct SessionStatus {
    room_name: String,
    nickname: Option<String>,
    peer_count: usize,
    synced: bool,
    ip_addrs: Vec<SocketAddr>,
}

/// State shared between the server and the session task.
struct SharedState {
    /// Message buffer for polling.
    messages: Mutex<Vec<MessageInfo>>,
    /// Last poll index for get_new_messages.
    last_poll_index: Mutex<usize>,
    /// Session command sender (None if not connected).
    session_cmd: RwLock<Option<mpsc::Sender<SessionCommand>>>,
    /// Current session status.
    status: RwLock<Option<SessionStatus>>,
}

impl SharedState {
    fn new() -> Self {
        Self {
            messages: Mutex::new(Vec::new()),
            last_poll_index: Mutex::new(0),
            session_cmd: RwLock::new(None),
            status: RwLock::new(None),
        }
    }
}

/// MCP server for DecentChat AI agent integration.
#[derive(Clone)]
pub struct McpServer {
    identity: Arc<Identity>,
    state: Arc<SharedState>,
    tool_router: ToolRouter<Self>,
}

impl McpServer {
    /// Create a new MCP server.
    pub fn new(identity: Identity, _config_dir: std::path::PathBuf) -> Self {
        Self {
            identity: Arc::new(identity),
            state: Arc::new(SharedState::new()),
            tool_router: Self::tool_router(),
        }
    }

    /// Run the MCP server with stdio transport.
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting MCP server");
        let service = self.serve(rmcp::transport::stdio()).await?;
        service.waiting().await?;
        Ok(())
    }

    /// Join a chat room.
    async fn do_join_room(&self, params: JoinRoomParams) -> JoinRoomResult {
        // Check if already connected.
        {
            let status = self.state.status.read().await;
            if let Some(ref s) = *status {
                return JoinRoomResult {
                    success: false,
                    room: s.room_name.clone(),
                    ticket: String::new(),
                    error: Some(format!("already connected to room: {}", s.room_name)),
                };
            }
        }

        // Parse ticket if provided.
        let (bootstrap, ticket_group) = if let Some(ref t) = params.ticket {
            match t.parse::<ConnectionTicket>() {
                Ok(ticket) => {
                    let peer = if ticket.addrs().is_empty() {
                        BootstrapPeer::new(ticket.node_id())
                    } else {
                        BootstrapPeer::with_addr(ticket.node_id(), ticket.addrs()[0])
                    };
                    (vec![peer], ticket.group().map(String::from))
                }
                Err(e) => {
                    return JoinRoomResult {
                        success: false,
                        room: String::new(),
                        ticket: String::new(),
                        error: Some(format!("invalid ticket: {}", e)),
                    };
                }
            }
        } else {
            (vec![], None)
        };

        // Determine room name.
        let room_name = match params.room.or(ticket_group) {
            Some(name) => name,
            None => {
                return JoinRoomResult {
                    success: false,
                    room: String::new(),
                    ticket: String::new(),
                    error: Some("room name or ticket required".to_string()),
                };
            }
        };

        // Create channel to receive join result.
        let (reply_tx, reply_rx) = oneshot::channel();

        // Spawn task to perform the join (to avoid Sync issues).
        let identity = Arc::clone(&self.identity);
        let state = Arc::clone(&self.state);
        let join_request = JoinRequest {
            room_name: room_name.clone(),
            bootstrap,
            nickname: params.nickname,
            reply: reply_tx,
        };

        tokio::spawn(async move {
            perform_join(identity, state, join_request).await;
        });

        // Wait for join result.
        match reply_rx.await {
            Ok(Ok(result)) => {
                // Store the command channel and status.
                {
                    let mut session_cmd = self.state.session_cmd.write().await;
                    *session_cmd = Some(result.cmd_tx);
                }
                {
                    let mut status = self.state.status.write().await;
                    *status = Some(result.initial_status);
                }
                // Clear message buffer.
                {
                    let mut messages = self.state.messages.lock().await;
                    messages.clear();
                    let mut last_poll_index = self.state.last_poll_index.lock().await;
                    *last_poll_index = 0;
                }
                info!("Joined room: {}", room_name);
                JoinRoomResult {
                    success: true,
                    room: room_name,
                    ticket: result.ticket,
                    error: None,
                }
            }
            Ok(Err(e)) => JoinRoomResult {
                success: false,
                room: room_name,
                ticket: String::new(),
                error: Some(e),
            },
            Err(_) => JoinRoomResult {
                success: false,
                room: room_name,
                ticket: String::new(),
                error: Some("join task failed".to_string()),
            },
        }
    }

    /// Send a message.
    async fn do_send_message(&self, params: SendMessageParams) -> SendMessageResult {
        if params.message.is_empty() {
            return SendMessageResult {
                success: false,
                message_id: None,
                error: Some("message cannot be empty".to_string()),
            };
        }

        let session_cmd = self.state.session_cmd.read().await;
        let cmd_tx = match session_cmd.as_ref() {
            Some(tx) => tx.clone(),
            None => {
                return SendMessageResult {
                    success: false,
                    message_id: None,
                    error: Some("not connected to any room".to_string()),
                };
            }
        };
        drop(session_cmd);

        let (reply_tx, reply_rx) = oneshot::channel();
        if cmd_tx
            .send(SessionCommand::SendMessage {
                content: params.message,
                reply: reply_tx,
            })
            .await
            .is_err()
        {
            return SendMessageResult {
                success: false,
                message_id: None,
                error: Some("session closed".to_string()),
            };
        }

        match reply_rx.await {
            Ok(Ok(msg)) => {
                let message_id = format!(
                    "{}:{}",
                    hex::encode(&msg.id.author.as_bytes()[..8]),
                    msg.id.seq
                );
                SendMessageResult {
                    success: true,
                    message_id: Some(message_id),
                    error: None,
                }
            }
            Ok(Err(e)) => SendMessageResult {
                success: false,
                message_id: None,
                error: Some(e),
            },
            Err(_) => SendMessageResult {
                success: false,
                message_id: None,
                error: Some("session closed".to_string()),
            },
        }
    }

    /// Set nickname.
    async fn do_set_nickname(&self, params: SetNicknameParams) -> SetNicknameResult {
        if params.nickname.is_empty() {
            return SetNicknameResult {
                success: false,
                nickname: None,
                error: Some("nickname cannot be empty".to_string()),
            };
        }

        let session_cmd = self.state.session_cmd.read().await;
        let cmd_tx = match session_cmd.as_ref() {
            Some(tx) => tx.clone(),
            None => {
                return SetNicknameResult {
                    success: false,
                    nickname: None,
                    error: Some("not connected to any room".to_string()),
                };
            }
        };
        drop(session_cmd);

        let (reply_tx, reply_rx) = oneshot::channel();
        if cmd_tx
            .send(SessionCommand::SetUsername {
                name: params.nickname.clone(),
                reply: reply_tx,
            })
            .await
            .is_err()
        {
            return SetNicknameResult {
                success: false,
                nickname: None,
                error: Some("session closed".to_string()),
            };
        }

        match reply_rx.await {
            Ok(Ok(())) => {
                // Update status.
                let mut status = self.state.status.write().await;
                if let Some(ref mut s) = *status {
                    s.nickname = Some(params.nickname.clone());
                }
                SetNicknameResult {
                    success: true,
                    nickname: Some(params.nickname),
                    error: None,
                }
            }
            Ok(Err(e)) => SetNicknameResult {
                success: false,
                nickname: None,
                error: Some(e),
            },
            Err(_) => SetNicknameResult {
                success: false,
                nickname: None,
                error: Some("session closed".to_string()),
            },
        }
    }

    /// Leave room.
    async fn do_leave_room(&self) -> LeaveRoomResult {
        let session_cmd = self.state.session_cmd.read().await;
        let cmd_tx = match session_cmd.as_ref() {
            Some(tx) => tx.clone(),
            None => {
                return LeaveRoomResult {
                    success: false,
                    error: Some("not connected to any room".to_string()),
                };
            }
        };
        drop(session_cmd);

        let (reply_tx, reply_rx) = oneshot::channel();
        if cmd_tx
            .send(SessionCommand::Leave { reply: reply_tx })
            .await
            .is_err()
        {
            return LeaveRoomResult {
                success: false,
                error: Some("session closed".to_string()),
            };
        }

        let room_name = {
            let status = self.state.status.read().await;
            status.as_ref().map(|s| s.room_name.clone()).unwrap_or_default()
        };

        match reply_rx.await {
            Ok(Ok(())) => {
                // Clear state.
                {
                    let mut session_cmd = self.state.session_cmd.write().await;
                    *session_cmd = None;
                }
                {
                    let mut status = self.state.status.write().await;
                    *status = None;
                }
                {
                    let mut messages = self.state.messages.lock().await;
                    messages.clear();
                }
                info!("Left room: {}", room_name);
                LeaveRoomResult {
                    success: true,
                    error: None,
                }
            }
            Ok(Err(e)) => LeaveRoomResult {
                success: false,
                error: Some(e),
            },
            Err(_) => {
                // Session task closed, clean up anyway.
                {
                    let mut session_cmd = self.state.session_cmd.write().await;
                    *session_cmd = None;
                }
                {
                    let mut status = self.state.status.write().await;
                    *status = None;
                }
                {
                    let mut messages = self.state.messages.lock().await;
                    messages.clear();
                }
                LeaveRoomResult {
                    success: true,
                    error: None,
                }
            }
        }
    }

    /// Get new messages.
    async fn do_get_new_messages(&self) -> GetNewMessagesResult {
        let buffer = self.state.messages.lock().await;
        let mut last_index = self.state.last_poll_index.lock().await;

        let new_messages: Vec<MessageInfo> = buffer.iter().skip(*last_index).cloned().collect();
        *last_index = buffer.len();

        GetNewMessagesResult {
            messages: new_messages,
        }
    }

    /// Get ticket.
    async fn do_get_ticket(&self) -> GetTicketResult {
        let status = self.state.status.read().await;
        let s = match status.as_ref() {
            Some(s) => s,
            None => {
                return GetTicketResult {
                    success: false,
                    ticket: None,
                    error: Some("not connected to any room".to_string()),
                };
            }
        };

        let ticket = if s.ip_addrs.is_empty() {
            ConnectionTicket::new(self.identity.node_id())
        } else {
            ConnectionTicket::with_addrs(self.identity.node_id(), s.ip_addrs.clone())
        }
        .with_group(&s.room_name);

        GetTicketResult {
            success: true,
            ticket: Some(ticket.to_string()),
            error: None,
        }
    }
}

/// Arguments for join_room tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct JoinRoomParams {
    /// Room name to join.
    #[serde(default)]
    pub room: Option<String>,
    /// Connection ticket (alternative to room name).
    #[serde(default)]
    pub ticket: Option<String>,
    /// Initial nickname.
    #[serde(default)]
    pub nickname: Option<String>,
}

/// Arguments for send_message tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SendMessageParams {
    /// Message content to send.
    pub message: String,
}

/// Arguments for set_nickname tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetNicknameParams {
    /// New nickname to use.
    pub nickname: String,
}

#[tool_router]
impl McpServer {
    /// Join a chat room by name or connection ticket.
    #[tool(description = "Join a chat room by name or connection ticket. Provide either 'room' (room name) or 'ticket' (connection ticket from another peer). Optionally set initial 'nickname'.")]
    async fn join_room(&self, Parameters(params): Parameters<JoinRoomParams>) -> String {
        let result = self.do_join_room(params).await;
        serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
    }

    /// Send a message to the current chat room.
    #[tool(description = "Send a message to the current chat room. Requires being connected to a room first.")]
    async fn send_message(&self, Parameters(params): Parameters<SendMessageParams>) -> String {
        let result = self.do_send_message(params).await;
        serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
    }

    /// Change display name.
    #[tool(description = "Change your display name (nickname) in the chat room.")]
    async fn set_nickname(&self, Parameters(params): Parameters<SetNicknameParams>) -> String {
        let result = self.do_set_nickname(params).await;
        serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
    }

    /// Leave the current chat room.
    #[tool(description = "Leave the current chat room and disconnect.")]
    async fn leave_room(&self) -> String {
        let result = self.do_leave_room().await;
        serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
    }

    /// Get new messages since the last poll.
    #[tool(description = "Get new messages since the last poll. Returns messages received since the previous call to this tool.")]
    async fn get_new_messages(&self) -> String {
        let result = self.do_get_new_messages().await;
        serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
    }

    /// Get a shareable connection ticket.
    #[tool(description = "Get a shareable connection ticket for the current room. Others can use this ticket to join.")]
    async fn get_ticket(&self) -> String {
        let result = self.do_get_ticket().await;
        serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "DecentChat MCP server for P2P chat. Use join_room to connect, \
                 send_message to chat, get_new_messages to receive messages, \
                 and leave_room to disconnect."
                    .to_string(),
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
            ..Default::default()
        }
    }

    async fn list_resources(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            resources: vec![
                RawResource {
                    uri: uri::MESSAGES.to_string(),
                    name: "Chat Messages".to_string(),
                    title: None,
                    description: Some("Recent messages in the current chat room".to_string()),
                    mime_type: Some("application/json".to_string()),
                    size: None,
                    icons: None,
                    meta: None,
                }
                .no_annotation(),
                RawResource {
                    uri: uri::USERS.to_string(),
                    name: "Chat Users".to_string(),
                    title: None,
                    description: Some("Online users in the current chat room".to_string()),
                    mime_type: Some("application/json".to_string()),
                    size: None,
                    icons: None,
                    meta: None,
                }
                .no_annotation(),
                RawResource {
                    uri: uri::STATUS.to_string(),
                    name: "Connection Status".to_string(),
                    title: None,
                    description: Some("Current connection status and room info".to_string()),
                    mime_type: Some("application/json".to_string()),
                    size: None,
                    icons: None,
                    meta: None,
                }
                .no_annotation(),
            ],
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri = &request.uri;
        match uri.as_str() {
            uri::MESSAGES => {
                let status = self.state.status.read().await;
                let buffer = self.state.messages.lock().await;

                let room = match status.as_ref() {
                    Some(s) => s.room_name.clone(),
                    None => "(not connected)".to_string(),
                };

                let resource = MessagesResource {
                    room,
                    messages: buffer.clone(),
                    total_count: buffer.len(),
                };

                Ok(ReadResourceResult {
                    contents: vec![ResourceContents::TextResourceContents {
                        uri: uri::MESSAGES.to_string(),
                        mime_type: Some("application/json".to_string()),
                        text: serde_json::to_string_pretty(&resource).unwrap_or_default(),
                        meta: None,
                    }],
                })
            }
            uri::USERS => {
                let session_cmd = self.state.session_cmd.read().await;

                let (room, users) = if let Some(cmd_tx) = session_cmd.as_ref() {
                    let status = self.state.status.read().await;
                    let room = status.as_ref().map(|s| s.room_name.clone()).unwrap_or_default();
                    drop(status);

                    let (reply_tx, reply_rx) = oneshot::channel();
                    if cmd_tx.send(SessionCommand::GetUsers { reply: reply_tx }).await.is_ok() {
                        match reply_rx.await {
                            Ok(users) => (room, users),
                            Err(_) => (room, vec![]),
                        }
                    } else {
                        (room, vec![])
                    }
                } else {
                    ("(not connected)".to_string(), vec![])
                };

                let resource = UsersResource { room, users };

                Ok(ReadResourceResult {
                    contents: vec![ResourceContents::TextResourceContents {
                        uri: uri::USERS.to_string(),
                        mime_type: Some("application/json".to_string()),
                        text: serde_json::to_string_pretty(&resource).unwrap_or_default(),
                        meta: None,
                    }],
                })
            }
            uri::STATUS => {
                let status = self.state.status.read().await;

                let status_info = match status.as_ref() {
                    Some(s) => StatusInfo {
                        connected: true,
                        room: Some(s.room_name.clone()),
                        nickname: s.nickname.clone(),
                        peer_count: s.peer_count,
                        synced: s.synced,
                    },
                    None => StatusInfo {
                        connected: false,
                        room: None,
                        nickname: None,
                        peer_count: 0,
                        synced: false,
                    },
                };

                let resource = StatusResource {
                    status: status_info,
                    node_id: hex::encode(self.identity.node_id().as_bytes()),
                };

                Ok(ReadResourceResult {
                    contents: vec![ResourceContents::TextResourceContents {
                        uri: uri::STATUS.to_string(),
                        mime_type: Some("application/json".to_string()),
                        text: serde_json::to_string_pretty(&resource).unwrap_or_default(),
                        meta: None,
                    }],
                })
            }
            _ => Err(McpError::resource_not_found(
                format!("unknown resource: {}", uri),
                None,
            )),
        }
    }
}

/// Perform the join operation in a separate task to avoid Sync issues.
async fn perform_join(
    identity: Arc<Identity>,
    state: Arc<SharedState>,
    request: JoinRequest,
) {
    let result = async {
        // Create transport and join.
        let config = QuicTransportConfig::default();
        let transport = QuicTransport::new(&identity, config)
            .await
            .map_err(|e| format!("failed to create transport: {}", e))?;

        let group_id = GroupId::new(&request.room_name);
        let subscription = transport
            .subscribe(&group_id, request.bootstrap)
            .await
            .map_err(|e| format!("failed to subscribe: {}", e))?;

        let (mut session, events) = GroupSession::new(
            group_id,
            identity.node_id(),
            subscription,
            SessionConfig::default(),
        );

        // Set nickname if provided.
        let nickname = request.nickname.clone();
        if let Some(ref name) = nickname
            && let Err(e) = session.set_username(name.clone()).await
        {
            warn!("failed to set nickname: {}", e);
        }

        // Get addresses for ticket.
        let endpoint_addr = transport.endpoint().addr();
        let ip_addrs: Vec<SocketAddr> = endpoint_addr.ip_addrs().copied().collect();

        // Generate ticket.
        let ticket = if ip_addrs.is_empty() {
            ConnectionTicket::new(identity.node_id())
        } else {
            ConnectionTicket::with_addrs(identity.node_id(), ip_addrs.clone())
        }
        .with_group(&request.room_name);

        // Create command channel.
        let (cmd_tx, cmd_rx) = mpsc::channel(32);

        // Initial status.
        let initial_status = SessionStatus {
            room_name: request.room_name.clone(),
            nickname,
            peer_count: session.peer_count(),
            synced: session.is_synced(),
            ip_addrs,
        };

        // Spawn session task.
        let state_clone = Arc::clone(&state);
        tokio::spawn(async move {
            run_session_task(session, events, transport, cmd_rx, state_clone).await;
        });

        Ok(JoinResult {
            ticket: ticket.to_string(),
            cmd_tx,
            initial_status,
        })
    }
    .await;

    let _ = request.reply.send(result);
}

/// Run the session task that owns the GroupSession.
async fn run_session_task(
    mut session: GroupSession,
    mut events: SessionEventReceiver,
    transport: QuicTransport,
    mut cmd_rx: mpsc::Receiver<SessionCommand>,
    state: Arc<SharedState>,
) {
    loop {
        tokio::select! {
            Some(cmd) = cmd_rx.recv() => {
                match cmd {
                    SessionCommand::SendMessage { content, reply } => {
                        let result = session.send_message(content).await;
                        let _ = reply.send(result.map_err(|e| e.to_string()));
                    }
                    SessionCommand::SetUsername { name, reply } => {
                        let result = session.set_username(name).await;
                        let _ = reply.send(result.map_err(|e| e.to_string()));
                    }
                    SessionCommand::Leave { reply } => {
                        let leave_result = session.leave().await;
                        if let Err(e) = leave_result {
                            warn!("error sending leave message: {}", e);
                        }
                        if let Err(e) = transport.shutdown().await {
                            warn!("error shutting down transport: {}", e);
                        }
                        let _ = reply.send(Ok(()));
                        break;
                    }
                    SessionCommand::GetUsers { reply } => {
                        let user_entries = session.state().users.all_entries();
                        let users: Vec<UserInfo> = user_entries
                            .into_iter()
                            .map(|(node, entry)| UserInfo {
                                node_id: hex::encode(node.as_bytes()),
                                name: entry.username.clone(),
                                last_seen: entry.last_seen,
                            })
                            .collect();
                        let _ = reply.send(users);
                    }
                }
            }
            Some(event) = events.recv() => {
                match &event {
                    ChatEvent::MessageReceived { message, .. } => {
                        let author_name = session.state().display_name(&message.author());
                        let info = MessageInfo {
                            author: author_name,
                            content: message.content.clone(),
                            timestamp: message.timestamp.wall_time,
                            id: format!(
                                "{}:{}",
                                hex::encode(&message.id.author.as_bytes()[..8]),
                                message.id.seq
                            ),
                        };
                        let mut buffer = state.messages.lock().await;
                        buffer.push(info);
                        if buffer.len() > MAX_MESSAGE_HISTORY {
                            buffer.remove(0);
                        }
                    }
                    ChatEvent::SyncCompleted { message_count, .. } => {
                        debug!("Sync completed with {} messages", message_count);
                        let messages: Vec<_> = session.state().messages.all_messages();
                        let session_state = session.state();
                        let new_buffer: Vec<MessageInfo> = messages
                            .iter()
                            .take(MAX_MESSAGE_HISTORY)
                            .map(|msg| {
                                let author_name = session_state.display_name(&msg.author());
                                MessageInfo {
                                    author: author_name,
                                    content: msg.content.clone(),
                                    timestamp: msg.timestamp.wall_time,
                                    id: format!(
                                        "{}:{}",
                                        hex::encode(&msg.id.author.as_bytes()[..8]),
                                        msg.id.seq
                                    ),
                                }
                            })
                            .collect();
                        let mut buffer = state.messages.lock().await;
                        *buffer = new_buffer;
                        // Update status.
                        let mut status = state.status.write().await;
                        if let Some(ref mut s) = *status {
                            s.synced = session.is_synced();
                            s.peer_count = session.peer_count();
                        }
                    }
                    ChatEvent::ConnectionChanged { .. } => {
                        // Update peer count.
                        let mut status = state.status.write().await;
                        if let Some(ref mut s) = *status {
                            s.peer_count = session.peer_count();
                        }
                    }
                    _ => {}
                }
            }
            result = session.process_event() => {
                if result.is_none() {
                    debug!("Session closed");
                    break;
                }
            }
        }
    }

    // Clean up state.
    {
        let mut session_cmd = state.session_cmd.write().await;
        *session_cmd = None;
    }
    {
        let mut status = state.status.write().await;
        *status = None;
    }
}
