use std::collections::HashMap;
use std::sync::Arc;
use futures::future;
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::Mutex;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::tcp::OwnedWriteHalf,
};

pub struct Server {
    listener: tokio::net::TcpListener,
    rooms: Arc<Mutex<HashMap<u32, Arc<Mutex<HashMap<String, OwnedWriteHalf>>>>>>,
}

impl Server {
    pub async fn new() -> Self {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
        Server {
            listener,
            rooms: Arc::new(Mutex::new(HashMap::new())),
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
                        .broadcast_message(room_id, &message, &addr.to_string())
                        .await;
                }
            });
        }
    }

    async fn broadcast_message(&self, room_id: u32, message: &str, from_addr: &String) {
        let rooms = self.rooms.lock().await;
        let writers_hm = rooms.get(&room_id).unwrap().clone();
        drop(rooms);

        let mut writers_hm = writers_hm.lock().await;
        let futures = writers_hm
            .iter_mut()
            .filter(|(peer_addr, _)| *peer_addr != from_addr)
            .map(|(_, w)| w.write_all(message.as_bytes()))
            .collect::<Vec<_>>();
        future::join_all(futures).await;
        drop(writers_hm);
    }

    async fn handle_room_join(
        &self,
        from_addr: &String,
        writer: OwnedWriteHalf,
        reader: &mut OwnedReadHalf,
    ) -> u32 {
        let mut buf = vec![0u8; 10];
        let bytes_read = reader.read(&mut buf).await.unwrap();
        let room_id = String::from_utf8(buf[..bytes_read - 2].to_vec())
            .unwrap()
            .parse::<u32>()
            .unwrap();
        let mut rooms = self.rooms.lock().await;
        let writers_hm = match rooms.get(&room_id) {
            Some(writers_hm) => writers_hm.clone(),
            None => {
                let writers_hm = Arc::new(Mutex::new(HashMap::new()));
                rooms.insert(room_id, writers_hm.clone());
                writers_hm
            }
        };
        drop(rooms);
        let mut writers_hm = writers_hm.lock().await;
        writers_hm.insert(from_addr.clone(), writer);
        drop(writers_hm);
        let message = format!("{:?} joined the room\n", from_addr);
        self.broadcast_message(room_id, &message, from_addr).await;
        println!("{:?} joined room {:?}", from_addr, room_id);
        return room_id;
    }

    async fn handle_disconnect(&self, room_id: u32, from_addr: String) {
        let rooms = self.rooms.lock().await;
        let writers_hm = rooms.get(&room_id).unwrap().clone();
        drop(rooms);
        writers_hm.lock().await.remove(&from_addr);
        if writers_hm.lock().await.is_empty() {
            self.rooms.lock().await.remove(&room_id);
            return;
        }
        let message = format!("{:?} left the room\n", from_addr);
        self.broadcast_message(room_id, &message, &from_addr).await;
        println!("{:?} left room {:?}", from_addr, room_id);
    }

    async fn send_welcome_prompt(&self, writer: &mut OwnedWriteHalf) {
        let rooms = self.rooms.lock().await;
        let room_ids_string = rooms
            .keys()
            .map(|id| id.to_string())
            .collect::<Vec<String>>()
            .join(", ");
        drop(rooms);

        let room_selection_prompt = format!(
            "Welcome to the server! Select the room you want to connect. Available rooms: {}\nEnter a number other than this to create a new room\n",
            room_ids_string
        );
        writer
            .write(room_selection_prompt.as_bytes())
            .await
            .unwrap();
    }
}
