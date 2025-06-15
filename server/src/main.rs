use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::{accept_async, tungstenite::Message, WebSocketStream};
use uuid::Uuid;

use verplant::{City, GameMessage, GameState, PlayerAction, SubwayMap};

type WebSocketSender = futures_util::stream::SplitSink<WebSocketStream<TcpStream>, Message>;

struct PlayerConnection {
    id: Uuid,
    _name: String,
    sender: Arc<Mutex<WebSocketSender>>,
}

struct GameSession {
    game_state: GameState,
    players: HashMap<Uuid, PlayerConnection>,
    subway_map: SubwayMap,
}

impl GameSession {
    fn new(city: City, conductor: Uuid) -> Self {
        let game_state = GameState::new(city.clone(), conductor);
        let subway_map = create_subway_map(&city);
        
        Self {
            game_state,
            players: HashMap::new(),
            subway_map,
        }
    }
    
    async fn add_player(&mut self, player: PlayerConnection) {
        self.game_state.add_player(player.id, &self.subway_map);
        self.players.insert(player.id, player);
    }
    
    async fn broadcast_message(&self, message: &GameMessage) {
        let message_text = serde_json::to_string(message).unwrap();
        
        for player in self.players.values() {
            if let Ok(mut sender) = player.sender.try_lock() {
                let _ = sender.send(Message::Text(message_text.clone())).await;
            }
        }
    }
    
    async fn send_to_player(&self, player_id: Uuid, message: &GameMessage) {
        if let Some(player) = self.players.get(&player_id) {
            if let Ok(mut sender) = player.sender.try_lock() {
                let message_text = serde_json::to_string(message).unwrap();
                let _ = sender.send(Message::Text(message_text)).await;
            }
        }
    }
    
    async fn handle_player_action(&mut self, player_id: Uuid, action: PlayerAction) {
        match self.game_state.process_player_action(player_id, action, &self.subway_map) {
            Ok(messages) => {
                for message in messages {
                    match &message {
                        GameMessage::LineCompleted { .. } => {
                            self.broadcast_message(&message).await;
                        },
                        _ => {
                            self.send_to_player(player_id, &message).await;
                        }
                    }
                }
                
                // Check if game ended
                if self.game_state.check_game_end() {
                    let scores = self.game_state.calculate_final_scores(&self.subway_map);
                    self.broadcast_message(&GameMessage::GameEnded { scores }).await;
                }
            },
            Err(error) => {
                self.send_to_player(player_id, &GameMessage::Error(error)).await;
            }
        }
    }
    
    async fn start_new_round(&mut self) {
        if let Some(card) = self.game_state.reveal_card() {
            self.broadcast_message(&GameMessage::CardRevealed(card)).await;
            self.broadcast_message(&GameMessage::GameState(self.game_state.clone())).await;
        }
    }
}

#[derive(Clone)]
struct GameServer {
    sessions: Arc<RwLock<HashMap<Uuid, Arc<Mutex<GameSession>>>>>,
}

impl GameServer {
    fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    async fn handle_connection(&self, stream: TcpStream, addr: SocketAddr) {
        println!("New WebSocket connection from: {}", addr);
        
        let ws_stream = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(e) => {
                println!("WebSocket connection error: {}", e);
                return;
            }
        };
        
        let (sender, mut receiver) = ws_stream.split();
        let sender = Arc::new(Mutex::new(sender));
        let mut player_id: Option<Uuid> = None;
        let mut game_session: Option<Arc<Mutex<GameSession>>> = None;
        
        while let Some(message) = receiver.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    if let Ok(game_message) = serde_json::from_str::<GameMessage>(&text) {
                        match game_message {
                            GameMessage::JoinGame { player_name, city } => {
                                let new_player_id = Uuid::new_v4();
                                player_id = Some(new_player_id);
                                
                                // Find or create game session for this city
                                let session = self.find_or_create_session(city, new_player_id).await;
                                
                                let player = PlayerConnection {
                                    id: new_player_id,
                                    _name: player_name,
                                    sender: sender.clone(),
                                };
                                
                                {
                                    let mut session_guard = session.lock().await;
                                    session_guard.add_player(player).await;
                                }
                                
                                game_session = Some(session.clone());
                                
                                // Send confirmation
                                let response = GameMessage::GameJoined {
                                    player_id: new_player_id,
                                    game_id: session.lock().await.game_state.id,
                                };
                                
                                if let Ok(mut sender_guard) = sender.try_lock() {
                                    let message_text = serde_json::to_string(&response).unwrap();
                                    let _ = sender_guard.send(Message::Text(message_text)).await;
                                }
                            },
                            
                            GameMessage::PlayerAction(action) => {
                                if let (Some(pid), Some(session)) = (player_id, &game_session) {
                                    let mut session_guard = session.lock().await;
                                    session_guard.handle_player_action(pid, action).await;
                                }
                            },
                            
                            GameMessage::StartGame => {
                                if let Some(session) = &game_session {
                                    let mut session_guard = session.lock().await;
                                    session_guard.start_new_round().await;
                                }
                            },
                            
                            _ => {
                                // Handle other message types as needed
                            }
                        }
                    }
                },
                Ok(Message::Close(_)) => {
                    println!("Client {} disconnected", addr);
                    break;
                },
                Err(e) => {
                    println!("WebSocket error for {}: {}", addr, e);
                    break;
                }
                _ => {}
            }
        }
    }
    
    async fn find_or_create_session(&self, city: City, conductor: Uuid) -> Arc<Mutex<GameSession>> {
        let sessions = self.sessions.read().await;
        
        // Try to find an existing session for this city with available slots
        for session in sessions.values() {
            let session_guard = session.lock().await;
            if session_guard.game_state.city == city && session_guard.players.len() < 6 {
                return session.clone();
            }
        }
        
        drop(sessions);
        
        // Create new session
        let new_session = Arc::new(Mutex::new(GameSession::new(city, conductor)));
        let session_id = new_session.lock().await.game_state.id;
        
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id, new_session.clone());
        
        new_session
    }
}

fn create_subway_map(city: &City) -> SubwayMap {
    // For now, create a simple Amsterdam map
    // This will be expanded with real subway data
    use std::collections::HashMap;
    use verplant::{LineId, Station, SubwayLine};
    
    let mut stations = HashMap::new();
    let mut lines = HashMap::new();
    
    match city {
        City::Amsterdam => {
            // Create a simple Amsterdam map for testing
            stations.insert("central".to_string(), Station {
                id: "central".to_string(),
                x: 100.0,
                y: 100.0,
                lines: vec![LineId("red".to_string()), LineId("blue".to_string())],
                is_transfer_hub: true,
            });
            
            stations.insert("dam".to_string(), Station {
                id: "dam".to_string(),
                x: 150.0,
                y: 100.0, 
                lines: vec![LineId("red".to_string())],
                is_transfer_hub: false,
            });
            
            stations.insert("museum".to_string(), Station {
                id: "museum".to_string(),
                x: 200.0,
                y: 100.0,
                lines: vec![LineId("red".to_string())],
                is_transfer_hub: false,
            });
            
            lines.insert(LineId("red".to_string()), SubwayLine {
                id: LineId("red".to_string()),
                color: "#FF0000".to_string(),
                stations: vec!["central".to_string(), "dam".to_string(), "museum".to_string()],
                is_ring: false,
                completion_points: (6, 3),
            });
            
            lines.insert(LineId("blue".to_string()), SubwayLine {
                id: LineId("blue".to_string()),
                color: "#0000FF".to_string(),
                stations: vec!["central".to_string()],
                is_ring: false,
                completion_points: (4, 2),
            });
        },
        _ => {
            // Placeholder for other cities
            stations.insert("test".to_string(), Station {
                id: "test".to_string(),
                x: 100.0,
                y: 100.0,
                lines: vec![LineId("test".to_string())],
                is_transfer_hub: false,
            });
            
            lines.insert(LineId("test".to_string()), SubwayLine {
                id: LineId("test".to_string()),
                color: "#000000".to_string(),
                stations: vec!["test".to_string()],
                is_ring: false,
                completion_points: (1, 1),
            });
        }
    }
    
    SubwayMap {
        city: city.clone(),
        stations,
        lines,
        special_stations: Vec::new(),
    }
}

#[tokio::main]
async fn main() {
    let server = GameServer::new();
    let listener = TcpListener::bind("127.0.0.1:8080").await.expect("Failed to bind");
    println!("WebSocket server listening on ws://127.0.0.1:8080");
    
    while let Ok((stream, addr)) = listener.accept().await {
        let server_clone = server.clone();
        tokio::spawn(async move {
            server_clone.handle_connection(stream, addr).await;
        });
    }
}