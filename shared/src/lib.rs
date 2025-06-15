use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum City {
    Amsterdam,
    Berlin,
    Paris,
    Madrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Card {
    Number(u8),         // Cards 1-5
    Six,                // Special card 6 (reshuffles deck)
    Express(u8),        // Express cards 2, 3, 4
    Transfer,           // Transfer opportunity card
    FreeRide,           // Free ride card
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct LineId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub lines: Vec<LineId>,
    pub is_transfer_hub: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubwayLine {
    pub id: LineId,
    pub color: String,
    pub stations: Vec<String>, // Station IDs in order
    pub is_ring: bool,        // For Berlin and Madrid ring lines
    pub completion_points: (u8, u8), // (first_to_complete, others)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubwayMap {
    pub city: City,
    pub stations: HashMap<String, Station>,
    pub lines: HashMap<LineId, SubwayLine>,
    pub special_stations: Vec<String>, // Paris/Madrid special stations
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSheet {
    pub player_id: Uuid,
    pub city: City,
    pub train_cars: HashMap<LineId, Vec<Option<String>>>, // Values in train car windows
    pub marked_stations: HashMap<String, StationMark>,
    pub completed_lines: Vec<LineId>,
    pub line_completion_status: HashMap<LineId, CompletionStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StationMark {
    Cross,
    TransferNumber(u8), // Number of connecting lines (doubled for scoring)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompletionStatus {
    FirstToComplete(u8), // Points for being first
    LaterCompletion(u8), // Points for completing after others
    NotCompleted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub id: Uuid,
    pub city: City,
    pub players: HashMap<Uuid, PlayerSheet>,
    pub current_card: Option<Card>,
    pub deck: Vec<Card>,
    pub discard_pile: Vec<Card>,
    pub round: u32,
    pub game_ended: bool,
    pub conductor: Uuid, // Player who shuffles cards
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlayerAction {
    ChooseLine { line_id: LineId, car_window_index: usize },
    MarkTransferStation { station_id: String },
    MarkFreeRideStation { station_id: String },
    CompleteLineAnnouncement { line_id: LineId },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameMessage {
    // Client to Server
    JoinGame { player_name: String, city: City },
    PlayerAction(PlayerAction),
    StartGame,
    
    // Server to Client
    GameJoined { player_id: Uuid, game_id: Uuid },
    GameState(GameState),
    CardRevealed(Card),
    PlayerActionResult { success: bool, message: String },
    LineCompleted { player_id: Uuid, line_id: LineId },
    GameEnded { scores: HashMap<Uuid, i32> },
    Error(String),
}

impl Card {
    pub fn create_deck() -> Vec<Card> {
        let mut deck = Vec::new();
        
        // Number cards 1-5 (2 of each)
        for num in 1..=5 {
            deck.push(Card::Number(num));
            deck.push(Card::Number(num));
        }
        
        // Special card 6 (1 card)
        deck.push(Card::Six);
        
        // Express cards (1 of each)
        deck.push(Card::Express(2));
        deck.push(Card::Express(3));
        deck.push(Card::Express(4));
        
        // Transfer and Free ride (1 of each)
        deck.push(Card::Transfer);
        deck.push(Card::FreeRide);
        
        deck
    }
    
    pub fn get_value(&self) -> Option<u8> {
        match self {
            Card::Number(n) | Card::Express(n) => Some(*n),
            Card::Six => Some(6),
            Card::Transfer | Card::FreeRide => None,
        }
    }
}

impl PlayerSheet {
    pub fn new(player_id: Uuid, city: City, subway_map: &SubwayMap) -> Self {
        let mut train_cars = HashMap::new();
        
        // Initialize train car windows for each line (typically 3-4 windows per line)
        for line_id in subway_map.lines.keys() {
            train_cars.insert(line_id.clone(), vec![None; 4]); // 4 windows per line
        }
        
        let mut line_completion_status = HashMap::new();
        for line_id in subway_map.lines.keys() {
            line_completion_status.insert(line_id.clone(), CompletionStatus::NotCompleted);
        }
        
        Self {
            player_id,
            city,
            train_cars,
            marked_stations: HashMap::new(),
            completed_lines: Vec::new(),
            line_completion_status,
        }
    }
    
    pub fn can_use_line(&self, line_id: &LineId) -> bool {
        if let Some(windows) = self.train_cars.get(line_id) {
            windows.iter().any(|w| w.is_none())
        } else {
            false
        }
    }
    
    pub fn add_card_to_line(&mut self, line_id: &LineId, card: &Card) -> Result<(), String> {
        if !self.can_use_line(line_id) {
            return Err("No empty windows available for this line".to_string());
        }
        
        if let Some(windows) = self.train_cars.get_mut(line_id) {
            for window in windows.iter_mut() {
                if window.is_none() {
                    *window = Some(match card {
                        Card::Transfer => "+".to_string(),
                        _ => card.get_value().map(|v| v.to_string()).unwrap_or("0".to_string()),
                    });
                    return Ok(());
                }
            }
        }
        
        Err("Could not add card to line".to_string())
    }
    
    pub fn mark_stations_from_line(&mut self, line_id: &LineId, card: &Card, subway_map: &SubwayMap) -> Result<Vec<String>, String> {
        let _line = subway_map.lines.get(line_id)
            .ok_or("Line not found")?;
            
        let mut marked_stations = Vec::new();
        
        match card {
            Card::FreeRide => {
                // Free ride: player can mark any empty station
                // This requires UI interaction, so we'll handle it differently
                Ok(marked_stations)
            },
            Card::Transfer => {
                // Find first empty station from train car and mark as transfer
                if let Some(station_id) = self.find_next_empty_station(line_id, subway_map)? {
                    let station = subway_map.stations.get(&station_id)
                        .ok_or("Station not found")?;
                    
                    let connection_count = station.lines.len() as u8;
                    self.marked_stations.insert(station_id.clone(), StationMark::TransferNumber(connection_count));
                    marked_stations.push(station_id);
                }
                Ok(marked_stations)
            },
            _ => {
                // Regular number or express card
                let value = card.get_value().unwrap_or(0);
                let is_express = matches!(card, Card::Express(_));
                
                let stations_to_mark = self.find_stations_to_mark(line_id, value, is_express, subway_map)?;
                
                for station_id in stations_to_mark.iter() {
                    self.marked_stations.insert(station_id.clone(), StationMark::Cross);
                    marked_stations.push(station_id.clone());
                }
                
                Ok(marked_stations)
            }
        }
    }
    
    fn find_next_empty_station(&self, line_id: &LineId, subway_map: &SubwayMap) -> Result<Option<String>, String> {
        let line = subway_map.lines.get(line_id)
            .ok_or("Line not found")?;
        
        // Start from the train car (beginning of line) and find first empty station
        for station_id in &line.stations {
            if !self.marked_stations.contains_key(station_id) {
                return Ok(Some(station_id.clone()));
            }
        }
        
        Ok(None)
    }
    
    fn find_stations_to_mark(&self, line_id: &LineId, value: u8, is_express: bool, subway_map: &SubwayMap) -> Result<Vec<String>, String> {
        let line = subway_map.lines.get(line_id)
            .ok_or("Line not found")?;
        
        let mut stations_to_mark = Vec::new();
        let mut remaining_marks = value;
        
        for station_id in &line.stations {
            if remaining_marks == 0 {
                break;
            }
            
            if self.marked_stations.contains_key(station_id) {
                if is_express {
                    // Express card: skip already marked stations
                    continue;
                } else {
                    // Regular card: stop at already marked station
                    break;
                }
            }
            
            stations_to_mark.push(station_id.clone());
            remaining_marks -= 1;
        }
        
        Ok(stations_to_mark)
    }
    
    pub fn check_line_completion(&mut self, line_id: &LineId, subway_map: &SubwayMap) -> bool {
        let line = subway_map.lines.get(line_id).unwrap();
        
        let all_marked = line.stations.iter()
            .all(|station_id| self.marked_stations.contains_key(station_id));
        
        if all_marked && !self.completed_lines.contains(line_id) {
            self.completed_lines.push(line_id.clone());
            return true;
        }
        
        false
    }
    
    pub fn calculate_score(&self, subway_map: &SubwayMap) -> i32 {
        let mut score = 0i32;
        
        // Points for completed lines
        for line_id in &self.completed_lines {
            if let Some(status) = self.line_completion_status.get(line_id) {
                match status {
                    CompletionStatus::FirstToComplete(points) => score += *points as i32,
                    CompletionStatus::LaterCompletion(points) => score += *points as i32,
                    CompletionStatus::NotCompleted => {}
                }
            }
        }
        
        // Double points for transfer stations
        for mark in self.marked_stations.values() {
            if let StationMark::TransferNumber(connections) = mark {
                score += (*connections as i32) * 2;
            }
        }
        
        // Penalty for empty stations (half the count, rounded down)
        let empty_stations = self.count_empty_stations(subway_map);
        score -= (empty_stations / 2) as i32;
        
        score
    }
    
    fn count_empty_stations(&self, subway_map: &SubwayMap) -> u32 {
        let mut total_stations = 0;
        let marked_stations = self.marked_stations.len() as u32;
        
        for line in subway_map.lines.values() {
            total_stations += line.stations.len() as u32;
        }
        
        total_stations - marked_stations
    }
}

impl GameState {
    pub fn new(city: City, conductor: Uuid) -> Self {
        let mut deck = Card::create_deck();
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        // Simple shuffle using system time as seed
        let mut hasher = DefaultHasher::new();
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        
        let seed = hasher.finish();
        Self::shuffle_deck(&mut deck, seed);
        
        Self {
            id: Uuid::new_v4(),
            city,
            players: HashMap::new(),
            current_card: None,
            deck,
            discard_pile: Vec::new(),
            round: 0,
            game_ended: false,
            conductor,
        }
    }
    
    fn shuffle_deck(deck: &mut [Card], seed: u64) {
        // Simple Fisher-Yates shuffle
        let mut rng_state = seed;
        
        for i in (1..deck.len()).rev() {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let j = (rng_state as usize) % (i + 1);
            deck.swap(i, j);
        }
    }
    
    pub fn add_player(&mut self, player_id: Uuid, subway_map: &SubwayMap) {
        let player_sheet = PlayerSheet::new(player_id, self.city.clone(), subway_map);
        self.players.insert(player_id, player_sheet);
    }
    
    pub fn draw_card(&mut self) -> Option<Card> {
        if let Some(card) = self.deck.pop() {
            Some(card)
        } else if !self.discard_pile.is_empty() {
            // Reshuffle discard pile into deck
            self.deck.append(&mut self.discard_pile);
            let mut hasher = DefaultHasher::new();
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
                .hash(&mut hasher);
            Self::shuffle_deck(&mut self.deck, hasher.finish());
            self.deck.pop()
        } else {
            None
        }
    }
    
    pub fn reveal_card(&mut self) -> Option<Card> {
        if let Some(card) = self.draw_card() {
            self.current_card = Some(card.clone());
            
            // Check if it's card 6 - reshuffle at end of round
            if matches!(card, Card::Six) {
                // This will be handled after all players make their moves
            }
            
            Some(card)
        } else {
            None
        }
    }
    
    pub fn handle_card_six(&mut self) {
        // Move all discard pile cards back to deck and shuffle
        self.deck.append(&mut self.discard_pile);
        if let Some(current) = self.current_card.take() {
            self.deck.push(current);
        }
        
        let mut hasher = DefaultHasher::new();
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .hash(&mut hasher);
        Self::shuffle_deck(&mut self.deck, hasher.finish());
    }
    
    pub fn process_player_action(&mut self, player_id: Uuid, action: PlayerAction, subway_map: &SubwayMap) -> Result<Vec<GameMessage>, String> {
        let mut messages = Vec::new();
        
        let current_card = self.current_card.as_ref()
            .ok_or("No card revealed")?;
        
        match action {
            PlayerAction::ChooseLine { line_id, car_window_index: _ } => {
                // Check if others have completed this line first
                let others_completed = self.players.values()
                    .filter(|p| p.player_id != player_id)
                    .any(|p| p.completed_lines.contains(&line_id));
                
                let player = self.players.get_mut(&player_id)
                    .ok_or("Player not found")?;
                
                // Add card value to train car window
                player.add_card_to_line(&line_id, current_card)?;
                
                // Mark stations based on card
                let marked_stations = player.mark_stations_from_line(&line_id, current_card, subway_map)?;
                
                // Check for line completion
                let line_completed = player.check_line_completion(&line_id, subway_map);
                if line_completed {
                    messages.push(GameMessage::LineCompleted { player_id, line_id: line_id.clone() });
                    
                    // Update completion status
                    if let Some(line) = subway_map.lines.get(&line_id) {
                        if !others_completed {
                            // First to complete
                            player.line_completion_status.insert(
                                line_id.clone(), 
                                CompletionStatus::FirstToComplete(line.completion_points.0)
                            );
                        } else {
                            // Later completion
                            player.line_completion_status.insert(
                                line_id.clone(), 
                                CompletionStatus::LaterCompletion(line.completion_points.1)
                            );
                        }
                    }
                }
                
                messages.push(GameMessage::PlayerActionResult { 
                    success: true, 
                    message: format!("Marked {} stations", marked_stations.len()) 
                });
            },
            
            PlayerAction::MarkTransferStation { station_id } => {
                if !matches!(current_card, Card::Transfer) {
                    return Err("Can only mark transfer station with transfer card".to_string());
                }
                
                let station = subway_map.stations.get(&station_id)
                    .ok_or("Station not found")?;
                
                let connection_count = station.lines.len() as u8;
                
                let player = self.players.get_mut(&player_id)
                    .ok_or("Player not found")?;
                
                player.marked_stations.insert(station_id, StationMark::TransferNumber(connection_count));
                
                messages.push(GameMessage::PlayerActionResult { 
                    success: true, 
                    message: format!("Marked transfer station with {} connections", connection_count) 
                });
            },
            
            PlayerAction::MarkFreeRideStation { station_id } => {
                if !matches!(current_card, Card::FreeRide) {
                    return Err("Can only mark free ride station with free ride card".to_string());
                }
                
                let player = self.players.get_mut(&player_id)
                    .ok_or("Player not found")?;
                
                if player.marked_stations.contains_key(&station_id) {
                    return Err("Station already marked".to_string());
                }
                
                player.marked_stations.insert(station_id, StationMark::Cross);
                
                messages.push(GameMessage::PlayerActionResult { 
                    success: true, 
                    message: "Marked free ride station".to_string() 
                });
            },
            
            PlayerAction::CompleteLineAnnouncement { line_id } => {
                messages.push(GameMessage::LineCompleted { player_id, line_id });
            },
        }
        
        Ok(messages)
    }
    
    pub fn check_game_end(&self) -> bool {
        // Game ends when all train car windows are filled
        self.players.values().all(|player| {
            player.train_cars.values().all(|windows| {
                windows.iter().all(|window| window.is_some())
            })
        })
    }
    
    pub fn calculate_final_scores(&self, subway_map: &SubwayMap) -> HashMap<Uuid, i32> {
        self.players.iter()
            .map(|(player_id, player)| (*player_id, player.calculate_score(subway_map)))
            .collect()
    }
    
    pub fn next_round(&mut self) {
        self.round += 1;
        
        // Move current card to discard pile
        if let Some(card) = self.current_card.take() {
            if matches!(card, Card::Six) {
                self.handle_card_six();
            } else {
                self.discard_pile.push(card);
            }
        }
        
        // Check if game should end
        if self.check_game_end() {
            self.game_ended = true;
        }
    }
}