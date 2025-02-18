use std::{collections::HashMap, sync::Arc};

use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf, sync::Mutex};
use futures::future;

pub struct Room {
    id: u32,
    writers: Arc<Mutex<HashMap<String, OwnedWriteHalf>>>,
    addr_name_map: Arc<Mutex<HashMap<String, String>>>,
}

impl Room {
    fn new(id: u32) -> Self {
        Room {
            id,
            writers: Arc::new(Mutex::new(HashMap::new())),
            addr_name_map: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn room_info(&self) -> String {
        let writers = self.writers.lock().await;
        let addr_name_map = self.addr_name_map.lock().await;
        let clients_info = writers
        .keys()
        .map(|addr| {
            let name = addr_name_map.get(addr).unwrap();
            format!("{}", name)
        })
        .collect::<Vec<String>>()
        .join("\n");
        drop(writers);
        drop(addr_name_map);
        let mut room_info = format!("Room ID: {}\n", self.id);
        if !clients_info.is_empty() {
            room_info.push_str("Members in this room\n");
        }
        room_info.push_str(&clients_info);
        room_info.push_str("\n==========\n");
        room_info
    }

    pub async fn broadcast_message(&self, message: &str, from_addr: &String) {

        let addr_name_map = self.addr_name_map.lock().await;
        let name = addr_name_map.get(from_addr).unwrap();
        let message = format!("{} > {}", name, message);

        let mut writers = self.writers.lock().await;
        let futures = writers
        .iter_mut()
        .filter(|(peer_addr, _)| *peer_addr != from_addr)
        .map(|(_, w)| w.write_all(message.as_bytes()))
        .collect::<Vec<_>>();
        future::join_all(futures).await;

        drop(writers);
        drop(addr_name_map);
    }

    pub async fn add_writer(&self, writer: OwnedWriteHalf, addr: String, name: String) {
        let mut writers = self.writers.lock().await;
        writers.insert(addr.clone(), writer);
        drop(writers);
        
        let mut addr_name_map = self.addr_name_map.lock().await;
        addr_name_map.insert(addr.clone(), name);
        drop(addr_name_map);
    }

    pub async fn remove_writer(&self, addr: &String) {
        let mut writers = self.writers.lock().await;
        writers.remove(addr);
        drop(writers);
    }

    pub async fn is_empty(&self) -> bool {
        let writers = self.writers.lock().await;
        let is_empty = writers.is_empty();
        drop(writers);
        is_empty
    }

    pub async fn get_name_from_addr(&self, addr: &String) -> String {
        let addr_name_map = self.addr_name_map.lock().await;
        let name = addr_name_map.get(addr).unwrap().clone();
        drop(addr_name_map);
        name
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
        drop(rooms);
    }

    pub async fn delete_room(&self, room_id: u32) {
        let mut rooms = self.rooms.lock().await;
        rooms.remove(&room_id);
        drop(rooms);
    }

    pub async fn get_room_ids_string(&self) -> String {
        let rooms = self.rooms.lock().await;
        let room_ids_string = rooms
            .keys()
            .map(|id| id.to_string())
            .collect::<Vec<String>>()
            .join(", ");
        drop(rooms);
        room_ids_string
    }
}