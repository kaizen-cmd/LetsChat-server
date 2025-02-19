mod arts;
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
        println!("{}\n Listening on port 8000", arts::art());
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
                    let bytes_read = match reader.read(&mut buf).await {
                        Ok(bytes_read) => bytes_read,
                        Err(_) => {
                            server_instance_clone
                                .handle_disconnect(room_id, &addr.to_string())
                                .await;
                            break;
                        }
                    };

                    if bytes_read == 0 {
                        server_instance_clone
                            .handle_disconnect(room_id, &addr.to_string())
                            .await;
                        break;
                    }

                    let message = String::from_utf8(buf[..bytes_read].to_vec()).unwrap();

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
        mut writer: OwnedWriteHalf,
        reader: &mut OwnedReadHalf,
    ) -> u32 {
        let mut buf = vec![0u8; 10];
        let bytes_read = reader.read(&mut buf).await.unwrap();
        let message: String = String::from_utf8(buf[..bytes_read].to_vec()).unwrap();
        let room_id_name: Vec<&str> = message.split(' ').collect::<Vec<_>>();

        let room_id = match room_id_name[0].parse::<u32>() {
            Ok(id) => id,
            Err(_) => {
                return 0;
            }
        };

        let name = room_id_name[1].to_string().trim().to_string();

        let room = match self.rooms_manager.get_room(room_id).await {
            Some(r) => r,
            None => {
                self.rooms_manager.create_room(room_id).await;
                self.rooms_manager.get_room(room_id).await.unwrap()
            }
        };

        writer
            .write_all(room.room_info().await.as_bytes())
            .await
            .unwrap();

        room.add_writer(writer, from_addr.clone(), name.clone())
            .await;

        let message = format!("{:?} joined the room {}", name, room_id);
        println!("{}", message);
        self.broadcast_message_to_room(room_id, &message, from_addr)
            .await;
        return room_id;
    }

    async fn handle_disconnect(&self, room_id: u32, from_addr: &String) {
        let room = match self.rooms_manager.get_room(room_id).await {
            Some(r) => r,
            None => return,
        };
        room.remove_writer(from_addr).await;

        let message = format!(
            "{:?} left the room\n",
            room.get_name_from_addr(&from_addr).await
        );
        self.broadcast_message_to_room(room_id, &message, &from_addr)
            .await;
        println!(
            "{:?} left room {:?}",
            room.get_name_from_addr(&from_addr).await,
            room_id
        );

        if room.is_empty().await {
            self.rooms_manager.delete_room(room_id).await;
            return;
        }
    }

    async fn send_welcome_prompt(&self, writer: &mut OwnedWriteHalf) {
        let room_ids_string = self.rooms_manager.get_room_ids_string().await;

        let room_selection_prompt = format!(
            "Welcome to the server! Select the room you want to connect.
Available rooms: {}
For eg: If you want to join room 12 as John, send '12 John'
New number creates a new room\n",
            room_ids_string
        );
        writer
            .write(room_selection_prompt.as_bytes())
            .await
            .unwrap();
    }
}
