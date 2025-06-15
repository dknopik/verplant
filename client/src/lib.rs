#![allow(deprecated)]

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    CanvasRenderingContext2d, HtmlCanvasElement, HtmlInputElement,
    HtmlSelectElement, MessageEvent, WebSocket, window,
};
use std::rc::Rc;
use std::cell::RefCell;

use verplant::{City, GameMessage, GameState, PlayerAction, LineId, SubwayMap};

// Set up panic hook for better error messages
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}

// Use wee_alloc as the global allocator
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen]
pub struct GameClient {
    websocket: Option<WebSocket>,
    game_state: Option<GameState>,
    player_id: Option<uuid::Uuid>,
    game_id: Option<uuid::Uuid>,
    subway_map: Option<SubwayMap>,
    #[allow(dead_code)]
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    city_select: HtmlSelectElement,
    name_input: HtmlInputElement,
}

#[wasm_bindgen]
impl GameClient {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<GameClient, JsValue> {
        let window = window().ok_or("No window")?;
        let document = window.document().ok_or("No document")?;
        
        let canvas = document
            .get_element_by_id("game-canvas")
            .ok_or("No canvas element")?
            .dyn_into::<HtmlCanvasElement>()?;
            
        let context = canvas
            .get_context("2d")?
            .ok_or("No 2d context")?
            .dyn_into::<CanvasRenderingContext2d>()?;
            
        let city_select = document
            .get_element_by_id("city-select")
            .ok_or("No city select")?
            .dyn_into::<HtmlSelectElement>()?;
            
        let name_input = document
            .get_element_by_id("player-name")
            .ok_or("No name input")?
            .dyn_into::<HtmlInputElement>()?;
        
        canvas.set_width(800);
        canvas.set_height(600);
        
        Ok(GameClient {
            websocket: None,
            game_state: None,
            player_id: None,
            game_id: None,
            subway_map: None,
            canvas,
            context,
            city_select,
            name_input,
        })
    }
    
    #[wasm_bindgen]
    pub fn connect_to_server(&mut self) -> Result<(), JsValue> {
        let ws = WebSocket::new("ws://127.0.0.1:8080")?;
        
        // Set up message handler
        let client_ref = Rc::new(RefCell::new(self as *mut GameClient));
        
        let onmessage_callback = {
            let client_ref = client_ref.clone();
            Closure::wrap(Box::new(move |e: MessageEvent| {
                if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                    let message_str = String::from(txt);
                    unsafe {
                        if let Ok(mut client_ref) = client_ref.try_borrow_mut() {
                            (**client_ref).handle_server_message(&message_str);
                        }
                    }
                }
            }) as Box<dyn FnMut(MessageEvent)>)
        };
        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        onmessage_callback.forget();
        
        // Set up connection handlers
        let onopen_callback = Closure::wrap(Box::new(move |_| {
            web_sys::console::log_1(&"Connected to server".into());
        }) as Box<dyn FnMut(JsValue)>);
        ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();
        
        let onerror_callback = Closure::wrap(Box::new(move |_e| {
            web_sys::console::error_1(&"WebSocket error".into());
        }) as Box<dyn FnMut(JsValue)>);
        ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
        onerror_callback.forget();
        
        self.websocket = Some(ws);
        Ok(())
    }
    
    #[wasm_bindgen]
    pub fn join_game(&self) -> Result<(), JsValue> {
        let player_name = self.name_input.value();
        let city_value = self.city_select.value();
        
        let city = match city_value.as_str() {
            "amsterdam" => City::Amsterdam,
            "berlin" => City::Berlin,
            "paris" => City::Paris,
            "madrid" => City::Madrid,
            _ => City::Amsterdam,
        };
        
        let message = GameMessage::JoinGame { player_name, city };
        self.send_message(&message)
    }
    
    #[wasm_bindgen]
    pub fn start_game(&self) -> Result<(), JsValue> {
        let message = GameMessage::StartGame;
        self.send_message(&message)
    }
    
    #[wasm_bindgen]
    pub fn choose_line(&self, line_id: &str, window_index: usize) -> Result<(), JsValue> {
        let action = PlayerAction::ChooseLine {
            line_id: LineId(line_id.to_string()),
            car_window_index: window_index,
        };
        let message = GameMessage::PlayerAction(action);
        self.send_message(&message)
    }
    
    #[wasm_bindgen]
    pub fn mark_transfer_station(&self, station_id: &str) -> Result<(), JsValue> {
        let action = PlayerAction::MarkTransferStation {
            station_id: station_id.to_string(),
        };
        let message = GameMessage::PlayerAction(action);
        self.send_message(&message)
    }
    
    #[wasm_bindgen]
    pub fn draw_game(&self) -> Result<(), JsValue> {
        // Clear canvas
        self.context.clear_rect(0.0, 0.0, 800.0, 600.0);
        
        if let (Some(game_state), Some(subway_map)) = (&self.game_state, &self.subway_map) {
            self.draw_subway_map(subway_map)?;
            self.draw_game_state(game_state)?;
        }
        
        Ok(())
    }
    
    fn draw_subway_map(&self, subway_map: &SubwayMap) -> Result<(), JsValue> {
        // Draw stations
        for station in subway_map.stations.values() {
            self.context.begin_path();
            self.context.arc(station.x as f64, station.y as f64, 8.0, 0.0, 2.0 * std::f64::consts::PI)?;
            
            if station.is_transfer_hub {
                self.context.set_fill_style(&"#FFD700".into()); // Gold for transfer stations
            } else {
                self.context.set_fill_style(&"#FFFFFF".into()); // White for regular stations
            }
            self.context.fill();
            
            self.context.set_stroke_style(&"#000000".into());
            self.context.set_line_width(2.0);
            self.context.stroke();
            
            // Draw station name
            self.context.set_fill_style(&"#000000".into());
            self.context.set_font("12px Arial");
            self.context.fill_text(&station.id, station.x as f64 + 12.0, station.y as f64 + 4.0)?;
        }
        
        // Draw subway lines
        for line in subway_map.lines.values() {
            self.context.begin_path();
            self.context.set_stroke_style(&line.color.as_str().into());
            self.context.set_line_width(4.0);
            
            let mut first = true;
            for station_id in &line.stations {
                if let Some(station) = subway_map.stations.get(station_id) {
                    if first {
                        self.context.move_to(station.x as f64, station.y as f64);
                        first = false;
                    } else {
                        self.context.line_to(station.x as f64, station.y as f64);
                    }
                }
            }
            self.context.stroke();
        }
        
        Ok(())
    }
    
    fn draw_game_state(&self, game_state: &GameState) -> Result<(), JsValue> {
        if let Some(player_id) = self.player_id {
            if let Some(player) = game_state.players.get(&player_id) {
                // Draw train car windows
                let mut y_offset = 20.0;
                for (line_id, windows) in &player.train_cars {
                    self.context.set_fill_style(&"#000000".into());
                    self.context.set_font("14px Arial");
                    self.context.fill_text(&format!("Line {}: ", line_id.0), 650.0, y_offset)?;
                    
                    for (i, window) in windows.iter().enumerate() {
                        let x = 650.0 + (i as f64 * 30.0);
                        let y = y_offset + 10.0;
                        
                        // Draw window box
                        self.context.set_stroke_style(&"#000000".into());
                        self.context.stroke_rect(x, y, 25.0, 25.0);
                        
                        // Draw window content
                        if let Some(value) = window {
                            self.context.set_fill_style(&"#000000".into());
                            self.context.fill_text(value, x + 8.0, y + 18.0)?;
                        }
                    }
                    
                    y_offset += 50.0;
                }
                
                // Draw marked stations on the map
                if let Some(subway_map) = &self.subway_map {
                    for (station_id, mark) in &player.marked_stations {
                        if let Some(station) = subway_map.stations.get(station_id) {
                            match mark {
                                verplant::StationMark::Cross => {
                                    // Draw X mark
                                    self.context.set_stroke_style(&"#FF0000".into());
                                    self.context.set_line_width(3.0);
                                    self.context.begin_path();
                                    self.context.move_to(station.x as f64 - 6.0, station.y as f64 - 6.0);
                                    self.context.line_to(station.x as f64 + 6.0, station.y as f64 + 6.0);
                                    self.context.move_to(station.x as f64 + 6.0, station.y as f64 - 6.0);
                                    self.context.line_to(station.x as f64 - 6.0, station.y as f64 + 6.0);
                                    self.context.stroke();
                                },
                                verplant::StationMark::TransferNumber(num) => {
                                    // Draw transfer number in square
                                    self.context.set_stroke_style(&"#0000FF".into());
                                    self.context.set_line_width(2.0);
                                    self.context.stroke_rect(station.x as f64 - 8.0, station.y as f64 - 8.0, 16.0, 16.0);
                                    
                                    self.context.set_fill_style(&"#0000FF".into());
                                    self.context.set_font("12px Arial");
                                    self.context.fill_text(&num.to_string(), station.x as f64 - 4.0, station.y as f64 + 4.0)?;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        // Draw current card
        if let Some(card) = &game_state.current_card {
            self.context.set_fill_style(&"#000000".into());
            self.context.set_font("16px Arial");
            let card_text = match card {
                verplant::Card::Number(n) | verplant::Card::Express(n) => {
                    format!("Current Card: {}", n)
                },
                verplant::Card::Six => {
                    "Current Card: 6".to_string()
                },
                verplant::Card::Transfer => "Current Card: Transfer (+)".to_string(),
                verplant::Card::FreeRide => "Current Card: Free Ride".to_string(),
            };
            self.context.fill_text(&card_text, 20.0, 550.0)?;
        }
        
        Ok(())
    }
    
    fn send_message(&self, message: &GameMessage) -> Result<(), JsValue> {
        if let Some(ws) = &self.websocket {
            let message_str = serde_json::to_string(message)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
            ws.send_with_str(&message_str)?;
        }
        Ok(())
    }
    
    fn handle_server_message(&mut self, message_str: &str) {
        if let Ok(message) = serde_json::from_str::<GameMessage>(message_str) {
            match message {
                GameMessage::GameJoined { player_id, game_id } => {
                    self.player_id = Some(player_id);
                    self.game_id = Some(game_id);
                    web_sys::console::log_1(&format!("Joined game {} as player {}", game_id, player_id).into());
                },
                GameMessage::GameState(state) => {
                    self.game_state = Some(state);
                    self.subway_map = Some(self.create_subway_map_for_city());
                    let _ = self.draw_game();
                },
                GameMessage::CardRevealed(card) => {
                    if let Some(ref mut state) = self.game_state {
                        state.current_card = Some(card);
                        let _ = self.draw_game();
                    }
                },
                GameMessage::LineCompleted { player_id, line_id } => {
                    web_sys::console::log_1(&format!("Player {} completed line {}", player_id, line_id.0).into());
                },
                GameMessage::GameEnded { scores } => {
                    let score_text = format!("Game ended! Scores: {:?}", scores);
                    web_sys::console::log_1(&score_text.into());
                },
                GameMessage::Error(error) => {
                    web_sys::console::error_1(&error.into());
                },
                _ => {}
            }
        }
    }
    
    fn create_subway_map_for_city(&self) -> SubwayMap {
        // Create the same subway map as the server
        use std::collections::HashMap;
        use verplant::{Station, SubwayLine};
        
        let mut stations = HashMap::new();
        let mut lines = HashMap::new();
        
        // Simple Amsterdam map for testing
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
        
        SubwayMap {
            city: City::Amsterdam,
            stations,
            lines,
            special_stations: Vec::new(),
        }
    }
}