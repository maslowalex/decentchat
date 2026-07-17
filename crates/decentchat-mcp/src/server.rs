//! MCP server implementation for DecentChat's Guardian room boundary.

use std::sync::Arc;

use decentchat_core::{ChatEvent, Message};
use decentchat_guardian::{GuardianNode, RoomSession, SessionConfig, SessionEventReceiver};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    AnnotateAble, ListResourcesResult, RawResource, ReadResourceRequestParams, ReadResourceResult,
    ResourceContents, ServerCapabilities, ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt, tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::sync::{Mutex, RwLock, mpsc, oneshot};
use tracing::{debug, info, warn};

use crate::resources::{MessagesResource, StatusResource, UsersResource, uri};
use crate::tools::{
    GetNewMessagesResult, GetTicketResult, JoinRoomResult, LeaveRoomResult, MessageInfo,
    SendMessageResult, SetNicknameResult, StatusInfo, UserInfo,
};

const MAX_MESSAGE_HISTORY: usize = 100;

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

struct JoinResult {
    room_name: String,
    ticket: String,
    cmd_tx: mpsc::Sender<SessionCommand>,
    initial_status: SessionStatus,
}

#[derive(Clone)]
struct SessionStatus {
    room_name: String,
    nickname: Option<String>,
    peer_count: usize,
    synced: bool,
    ticket: String,
}

struct SharedState {
    messages: Mutex<Vec<MessageInfo>>,
    last_poll_index: Mutex<usize>,
    session_cmd: RwLock<Option<mpsc::Sender<SessionCommand>>>,
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

#[derive(Clone)]
pub struct McpServer {
    node: GuardianNode,
    state: Arc<SharedState>,
    tool_router: ToolRouter<Self>,
}

impl McpServer {
    pub fn new(node: GuardianNode) -> Self {
        Self {
            node,
            state: Arc::new(SharedState::new()),
            tool_router: Self::tool_router(),
        }
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting MCP server");
        let service = self.serve(rmcp::transport::stdio()).await?;
        service.waiting().await?;
        Ok(())
    }

    async fn do_join_room(&self, params: JoinRoomParams) -> JoinRoomResult {
        if let Some(status) = self.state.status.read().await.as_ref() {
            return JoinRoomResult {
                success: false,
                room: status.room_name.clone(),
                ticket: String::new(),
                error: Some(format!("already connected to room: {}", status.room_name)),
            };
        }
        if params.room.is_some() == params.ticket.is_some() {
            return JoinRoomResult {
                success: false,
                room: String::new(),
                ticket: String::new(),
                error: Some("provide exactly one of room or ticket".into()),
            };
        }
        if params
            .room
            .as_deref()
            .is_some_and(|room| room.trim().is_empty())
        {
            return JoinRoomResult {
                success: false,
                room: String::new(),
                ticket: String::new(),
                error: Some("room name cannot be empty".into()),
            };
        }

        self.state.messages.lock().await.clear();
        *self.state.last_poll_index.lock().await = 0;

        match perform_join(
            self.node.clone(),
            Arc::clone(&self.state),
            params.room,
            params.ticket,
            params.nickname,
        )
        .await
        {
            Ok(result) => {
                *self.state.session_cmd.write().await = Some(result.cmd_tx);
                *self.state.status.write().await = Some(result.initial_status);
                JoinRoomResult {
                    success: true,
                    room: result.room_name,
                    ticket: result.ticket,
                    error: None,
                }
            }
            Err(error) => JoinRoomResult {
                success: false,
                room: String::new(),
                ticket: String::new(),
                error: Some(error),
            },
        }
    }

    async fn do_send_message(&self, params: SendMessageParams) -> SendMessageResult {
        if params.message.is_empty() {
            return SendMessageResult {
                success: false,
                message_id: None,
                error: Some("message cannot be empty".into()),
            };
        }
        let Some(tx) = self.state.session_cmd.read().await.clone() else {
            return SendMessageResult {
                success: false,
                message_id: None,
                error: Some("not connected to any room".into()),
            };
        };
        let (reply_tx, reply_rx) = oneshot::channel();
        if tx
            .send(SessionCommand::SendMessage {
                content: params.message,
                reply: reply_tx,
            })
            .await
            .is_err()
        {
            return closed_send_result();
        }
        match reply_rx.await {
            Ok(Ok(message)) => SendMessageResult {
                success: true,
                message_id: Some(message.id.to_string()),
                error: None,
            },
            Ok(Err(error)) => SendMessageResult {
                success: false,
                message_id: None,
                error: Some(error),
            },
            Err(_) => closed_send_result(),
        }
    }

    async fn do_set_nickname(&self, params: SetNicknameParams) -> SetNicknameResult {
        if params.nickname.trim().is_empty() {
            return SetNicknameResult {
                success: false,
                nickname: None,
                error: Some("nickname cannot be empty".into()),
            };
        }
        let Some(tx) = self.state.session_cmd.read().await.clone() else {
            return SetNicknameResult {
                success: false,
                nickname: None,
                error: Some("not connected to any room".into()),
            };
        };
        let (reply_tx, reply_rx) = oneshot::channel();
        if tx
            .send(SessionCommand::SetUsername {
                name: params.nickname.clone(),
                reply: reply_tx,
            })
            .await
            .is_err()
        {
            return closed_nickname_result();
        }
        match reply_rx.await {
            Ok(Ok(())) => {
                if let Some(status) = self.state.status.write().await.as_mut() {
                    status.nickname = Some(params.nickname.clone());
                }
                SetNicknameResult {
                    success: true,
                    nickname: Some(params.nickname),
                    error: None,
                }
            }
            Ok(Err(error)) => SetNicknameResult {
                success: false,
                nickname: None,
                error: Some(error),
            },
            Err(_) => closed_nickname_result(),
        }
    }

    async fn do_leave_room(&self) -> LeaveRoomResult {
        let Some(tx) = self.state.session_cmd.read().await.clone() else {
            return LeaveRoomResult {
                success: false,
                error: Some("not connected to any room".into()),
            };
        };
        let (reply_tx, reply_rx) = oneshot::channel();
        if tx
            .send(SessionCommand::Leave { reply: reply_tx })
            .await
            .is_err()
        {
            clear_connected_state(&self.state).await;
            return LeaveRoomResult {
                success: true,
                error: None,
            };
        }
        match reply_rx.await {
            Ok(Err(error)) => LeaveRoomResult {
                success: false,
                error: Some(error),
            },
            _ => {
                clear_connected_state(&self.state).await;
                LeaveRoomResult {
                    success: true,
                    error: None,
                }
            }
        }
    }

    async fn do_get_new_messages(&self) -> GetNewMessagesResult {
        let buffer = self.state.messages.lock().await;
        let mut last_index = self.state.last_poll_index.lock().await;
        let messages = buffer.iter().skip(*last_index).cloned().collect();
        *last_index = buffer.len();
        GetNewMessagesResult { messages }
    }

    async fn do_get_ticket(&self) -> GetTicketResult {
        match self.state.status.read().await.as_ref() {
            Some(status) => GetTicketResult {
                success: true,
                ticket: Some(status.ticket.clone()),
                error: None,
            },
            None => GetTicketResult {
                success: false,
                ticket: None,
                error: Some("not connected to any room".into()),
            },
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct JoinRoomParams {
    #[serde(default)]
    pub room: Option<String>,
    #[serde(default)]
    pub ticket: Option<String>,
    #[serde(default)]
    pub nickname: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SendMessageParams {
    pub message: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetNicknameParams {
    pub nickname: String,
}

#[tool_router]
impl McpServer {
    #[tool(
        description = "Join a Guardian room. Provide exactly one of 'room' (create) or raw Guardian 'ticket' (import). Optionally set 'nickname'."
    )]
    async fn join_room(&self, Parameters(params): Parameters<JoinRoomParams>) -> String {
        serde_json::to_string(&self.do_join_room(params).await).unwrap_or_else(|_| "{}".into())
    }

    #[tool(description = "Send a message to the current chat room.")]
    async fn send_message(&self, Parameters(params): Parameters<SendMessageParams>) -> String {
        serde_json::to_string(&self.do_send_message(params).await).unwrap_or_else(|_| "{}".into())
    }

    #[tool(description = "Change your display name in the current room.")]
    async fn set_nickname(&self, Parameters(params): Parameters<SetNicknameParams>) -> String {
        serde_json::to_string(&self.do_set_nickname(params).await).unwrap_or_else(|_| "{}".into())
    }

    #[tool(description = "Leave the current room gracefully.")]
    async fn leave_room(&self) -> String {
        serde_json::to_string(&self.do_leave_room().await).unwrap_or_else(|_| "{}".into())
    }

    #[tool(description = "Get messages received since the previous poll.")]
    async fn get_new_messages(&self) -> String {
        serde_json::to_string(&self.do_get_new_messages().await).unwrap_or_else(|_| "{}".into())
    }

    #[tool(description = "Get the current room's raw Guardian DocTicket.")]
    async fn get_ticket(&self) -> String {
        serde_json::to_string(&self.do_get_ticket().await).unwrap_or_else(|_| "{}".into())
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "DecentChat MCP server backed by Guardian DB. Create a room by name or import a raw Guardian ticket, then send and poll messages."
                    .into(),
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
                resource(
                    uri::MESSAGES,
                    "Chat Messages",
                    "Recent messages in the current room",
                ),
                resource(uri::USERS, "Chat Users", "Members in the current room"),
                resource(
                    uri::STATUS,
                    "Connection Status",
                    "Current Guardian room status",
                ),
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
        let text = match request.uri.as_str() {
            uri::MESSAGES => {
                let room = current_room(&self.state).await;
                let messages = self.state.messages.lock().await.clone();
                serde_json::to_string_pretty(&MessagesResource {
                    room,
                    total_count: messages.len(),
                    messages,
                })
                .unwrap_or_default()
            }
            uri::USERS => {
                let room = current_room(&self.state).await;
                let users = request_users(&self.state).await;
                serde_json::to_string_pretty(&UsersResource { room, users }).unwrap_or_default()
            }
            uri::STATUS => {
                let status = self.state.status.read().await;
                let info = status.as_ref().map_or(
                    StatusInfo {
                        connected: false,
                        room: None,
                        nickname: None,
                        peer_count: 0,
                        synced: false,
                    },
                    |status| StatusInfo {
                        connected: true,
                        room: Some(status.room_name.clone()),
                        nickname: status.nickname.clone(),
                        peer_count: status.peer_count,
                        synced: status.synced,
                    },
                );
                serde_json::to_string_pretty(&StatusResource {
                    status: info,
                    node_id: self.node.node_id().to_hex(),
                })
                .unwrap_or_default()
            }
            unknown => {
                return Err(McpError::resource_not_found(
                    format!("unknown resource: {unknown}"),
                    None,
                ));
            }
        };
        Ok(ReadResourceResult {
            contents: vec![ResourceContents::TextResourceContents {
                uri: request.uri,
                mime_type: Some("application/json".into()),
                text,
                meta: None,
            }],
        })
    }
}

async fn perform_join(
    node: GuardianNode,
    state: Arc<SharedState>,
    room: Option<String>,
    ticket: Option<String>,
    nickname: Option<String>,
) -> Result<JoinResult, String> {
    let (mut session, events) = match (room, ticket) {
        (Some(room), None) => node.create_room(&room, SessionConfig::default()).await,
        (None, Some(ticket)) => node.join_room(&ticket, SessionConfig::default()).await,
        _ => unreachable!(),
    }
    .map_err(|error| error.to_string())?;

    if let Some(name) = nickname.clone() {
        session
            .set_username(name)
            .await
            .map_err(|error| error.to_string())?;
    }
    let ticket = session
        .share_ticket()
        .await
        .map_err(|error| error.to_string())?;
    let room_name = session.state().metadata.name.clone();
    let initial_status = SessionStatus {
        room_name: room_name.clone(),
        nickname,
        peer_count: session.peer_count(),
        synced: session.is_synced(),
        ticket: ticket.clone(),
    };
    let (cmd_tx, cmd_rx) = mpsc::channel(32);
    tokio::spawn(run_session_task(session, events, cmd_rx, state));
    Ok(JoinResult {
        room_name,
        ticket,
        cmd_tx,
        initial_status,
    })
}

async fn run_session_task(
    mut session: RoomSession,
    mut events: SessionEventReceiver,
    mut commands: mpsc::Receiver<SessionCommand>,
    state: Arc<SharedState>,
) {
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(SessionCommand::SendMessage { content, reply }) => {
                    let result = session.send_message(content).await.map_err(|error| error.to_string());
                    let _ = reply.send(result);
                }
                Some(SessionCommand::SetUsername { name, reply }) => {
                    let result = session.set_username(name).await.map_err(|error| error.to_string());
                    let _ = reply.send(result);
                }
                Some(SessionCommand::GetUsers { reply }) => {
                    let users = session.state().members.values().filter(|member| !member.offline).map(|member| UserInfo {
                        node_id: member.node_id.to_hex(),
                        name: session.state().display_name(&member.node_id),
                        last_seen: Some(member.heartbeat_at_ms),
                    }).collect();
                    let _ = reply.send(users);
                }
                Some(SessionCommand::Leave { reply }) => {
                    let result = session.leave().await.map_err(|error| error.to_string());
                    let _ = reply.send(result);
                    break;
                }
                None => break,
            },
            event = events.recv() => match event {
                Some(event) => handle_session_event(&session, &state, event).await,
                None => break,
            },
            projected = session.process_event() => match projected {
                Some(Ok(())) => {}
                Some(Err(error)) => warn!(%error, "Guardian room projection failed"),
                None => break,
            }
        }
    }
    clear_connected_state(&state).await;
    debug!("Guardian MCP room task stopped");
}

async fn handle_session_event(session: &RoomSession, state: &SharedState, event: ChatEvent) {
    match event {
        ChatEvent::MessageReceived { message, .. } => {
            push_message(state, message_info(session, &message)).await;
        }
        ChatEvent::SyncCompleted { .. } => {
            let history = session
                .state()
                .messages
                .iter()
                .rev()
                .take(MAX_MESSAGE_HISTORY)
                .rev()
                .map(|message| message_info(session, message))
                .collect();
            *state.messages.lock().await = history;
        }
        _ => {}
    }
    if let Some(status) = state.status.write().await.as_mut() {
        status.peer_count = session.peer_count();
        status.synced = session.is_synced();
    }
}

fn message_info(session: &RoomSession, message: &Message) -> MessageInfo {
    MessageInfo {
        author: session.state().display_name(&message.author),
        content: message.content.clone(),
        timestamp: message.sent_at_ms,
        id: message.id.to_string(),
    }
}

async fn push_message(state: &SharedState, message: MessageInfo) {
    let mut messages = state.messages.lock().await;
    messages.push(message);
    if messages.len() > MAX_MESSAGE_HISTORY {
        messages.remove(0);
    }
}

async fn request_users(state: &SharedState) -> Vec<UserInfo> {
    let Some(tx) = state.session_cmd.read().await.clone() else {
        return Vec::new();
    };
    let (reply_tx, reply_rx) = oneshot::channel();
    if tx
        .send(SessionCommand::GetUsers { reply: reply_tx })
        .await
        .is_err()
    {
        return Vec::new();
    }
    reply_rx.await.unwrap_or_default()
}

async fn current_room(state: &SharedState) -> String {
    state
        .status
        .read()
        .await
        .as_ref()
        .map(|status| status.room_name.clone())
        .unwrap_or_else(|| "(not connected)".into())
}

async fn clear_connected_state(state: &SharedState) {
    *state.session_cmd.write().await = None;
    *state.status.write().await = None;
    state.messages.lock().await.clear();
    *state.last_poll_index.lock().await = 0;
}

fn resource(uri: &str, name: &str, description: &str) -> rmcp::model::Resource {
    RawResource {
        uri: uri.into(),
        name: name.into(),
        title: None,
        description: Some(description.into()),
        mime_type: Some("application/json".into()),
        size: None,
        icons: None,
        meta: None,
    }
    .no_annotation()
}

fn closed_send_result() -> SendMessageResult {
    SendMessageResult {
        success: false,
        message_id: None,
        error: Some("session closed".into()),
    }
}

fn closed_nickname_result() -> SetNicknameResult {
    SetNicknameResult {
        success: false,
        nickname: None,
        error: Some("session closed".into()),
    }
}
