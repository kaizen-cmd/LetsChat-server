use std::{collections::HashMap, sync::Arc};

use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf, sync::Mutex};
use futures::future;

pub struct Room {
    _id: u32,
    pub writers: Arc<Mutex<HashMap<String, OwnedWriteHalf>>>,
    pub addr_name_map: Arc<Mutex<HashMap<String, String>>>,
}

impl Room {
    fn new(id: u32) -> Self {
        Room {
            _id: id,
            writers: Arc::new(Mutex::new(HashMap::new())),
            addr_name_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn broadcast_message(&self, message: &str, from_addr: &String) {
        let mut writers = self.writers.lock().await;
        let futures = writers
            .iter_mut()
            .filter(|(peer_addr, _)| *peer_addr != from_addr)
            .map(|(_, w)| w.write_all(message.as_bytes()))
            .collect::<Vec<_>>();
        future::join_all(futures).await;
        drop(writers);
    }
}

pub struct RoomsManager {
    pub rooms: Arc<Mutex<HashMap<u32, Arc<Room>>>>,
}

impl RoomsManager {
    pub fn new() -> Self {
        RoomsManager {
            rooms: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn get_room(&self, room_id: u32) -> Option<Arc<Room>> {
        let rooms = self.rooms.lock().await;
        let room = rooms.get(&room_id).map(|r| r.clone());
        drop(rooms);
        room
    }

    pub async fn create_room(&self, room_id: u32) {
        let mut rooms = self.rooms.lock().await;
        rooms.insert(room_id, Arc::new(Room::new(room_id)));
    }
}