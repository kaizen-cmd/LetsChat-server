mod rooms;

use rooms::RoomsManager;

use std::sync::Arc;
use tokio::net::tcp::OwnedReadHalf;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::tcp::OwnedWriteHalf,
};


pub struct Server {
    listener: tokio::net::TcpListener,
    rooms_manager: RoomsManager,
}

impl Server {
    pub async fn new() -> Self {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
        Server {
            listener,
            rooms_manager: RoomsManager::new(),
        }
    }

    pub async fn start(server_instance: Arc<Server>) {
        println!("Listening on port 8000");
        loop {
            let (socket, addr) = server_instance.listener.accept().await.unwrap();
            let (mut reader, mut writer) = socket.into_split();
            let server_instance_clone = server_instance.clone();

            tokio::spawn(async move {
                server_instance_clone.send_welcome_prompt(&mut writer).await;
                let room_id = server_instance_clone
                    .handle_room_join(&addr.to_string(), writer, &mut reader)
                    .await;

                loop {
                    let mut buf = vec![0u8; 1024];
                    let bytes_read = reader.read(&mut buf).await.unwrap();

                    if bytes_read == 0 {
                        server_instance_clone
                            .handle_disconnect(room_id, addr.to_string())
                            .await;
                        break;
                    }

                    let message = format!(
                        "{}: {}",
                        addr,
                        String::from_utf8(buf[..bytes_read].to_vec()).unwrap()
                    );

                    server_instance_clone
                        .broadcast_message_to_room(room_id, &message, &addr.to_string())
                        .await;
                }
            });
        }
    }

    async fn broadcast_message_to_room(&self, room_id: u32, message: &str, from_addr: &String) {
        let room = self.rooms_manager.get_room(room_id).await.unwrap();
        room.broadcast_message(message, from_addr).await;
    }

    async fn handle_room_join(
        &self,
        from_addr: &String,
        writer: OwnedWriteHalf,
        reader: &mut OwnedReadHalf,
    ) -> u32 {
        let mut buf = vec![0u8; 10];
        let bytes_read = reader.read(&mut buf).await.unwrap();
        let message: String = String::from_utf8(buf[..bytes_read].to_vec()).unwrap();
        let room_id_name: Vec<&str> = message.split(' ').collect::<Vec<_>>();

        let room_id = room_id_name[0]
            .parse::<u32>()
            .unwrap();

        let name = room_id_name[1].to_string();

        let room = match self.rooms_manager.get_room(room_id).await {
            Some(r) => r,
            None => {
                self.rooms_manager.create_room(room_id).await;
                self.rooms_manager.get_room(room_id).await.unwrap()
            }
        };

        let mut addr_name_map = room.addr_name_map.lock().await;
        addr_name_map.insert(from_addr.clone(), name);
        drop(addr_name_map);

        let mut writers = room.writers.lock().await;
        writers.insert(from_addr.clone(), writer);
        drop(writers);

        let message = format!("{:?} joined the room\n", from_addr);
        self.broadcast_message_to_room(room_id, &message, from_addr).await;
        println!("{:?} joined room {:?}", from_addr, room_id);
        return room_id;
    }

    async fn handle_disconnect(&self, room_id: u32, from_addr: String) {
        let rooms = self.rooms_manager.rooms.lock().await;
        let room = self.rooms_manager.get_room(room_id).await.unwrap();
        drop(rooms);
        room.writers.lock().await.remove(&from_addr);
        if room.writers.lock().await.is_empty() {
            self.rooms_manager.rooms.lock().await.remove(&room_id);
            return;
        }
        let message = format!("{:?} left the room\n", from_addr);
        self.broadcast_message_to_room(room_id, &message, &from_addr).await;
        println!("{:?} left room {:?}", from_addr, room_id);
    }

    async fn send_welcome_prompt(&self, writer: &mut OwnedWriteHalf) {
        let rooms = self.rooms_manager.rooms.lock().await;
        let room_ids_string = rooms
            .keys()
            .map(|id| id.to_string())
            .collect::<Vec<String>>()
            .join(", ");
        drop(rooms);

        let room_selection_prompt = format!(
            "Welcome to the server! Select the room you want to connect.\n
            Available rooms: {}\n
            For eg: If you want to join room 12 as John, send '12 John'\n
            New number creates a new room\n",
            room_ids_string
        );
        writer
            .write(room_selection_prompt.as_bytes())
            .await
            .unwrap();
    }
}
