use core::str;
use std::{collections::HashMap, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{tcp::OwnedWriteHalf, TcpListener},
    sync::Mutex,
};

#[tokio::main]
async fn main() {
    let server = TcpListener::bind("127.0.0.1:8000").await.unwrap();

    let rooms: Arc<Mutex<HashMap<u32, Vec<Arc<Mutex<OwnedWriteHalf>>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let last_room_id = Arc::new(Mutex::new(0));

    loop {
        let (client, _addr) = server.accept().await.unwrap();
        println!("CLIENT JOINED: {:?}", _addr);

        let (mut reader, writer) = client.into_split();
        let writer_arc = Arc::new(Mutex::new(writer));

        let rooms = Arc::clone(&rooms);
        let last_room_id = Arc::clone(&last_room_id);
        let writer_arc_clone = Arc::clone(&writer_arc);

        // Send room selection prompt
        {
            let mut writer_locked = writer_arc_clone.lock().await;
            writer_locked
                .write_all(b"Which room do you want to join?\n")
                .await
                .unwrap();

            let mut list_of_rooms_string = String::new();
            let locked_rooms = rooms.lock().await;
            for k in locked_rooms.keys() {
                list_of_rooms_string.push_str(&k.to_string());
                list_of_rooms_string.push('\n');
            }

            writer_locked
                .write_all(list_of_rooms_string.as_bytes())
                .await
                .unwrap();
            writer_locked
                .write_all(b"Type 'new' to create a new room\n")
                .await
                .unwrap();
        }

        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            loop {
                let bytes_read = reader.read(&mut buf).await.unwrap();
                if bytes_read == 0 {
                    println!("Client disconnected {:?}", _addr);
                    break;
                }

                let res = str::from_utf8(&buf[0..bytes_read]).unwrap().trim(); // Trim spaces and newlines

                if res == "new" {
                    let mut last_room_locked = last_room_id.lock().await;
                    *last_room_locked += 1;
                    let new_room_id = *last_room_locked;
                    drop(last_room_locked); // Release lock early

                    println!("New room {} created", new_room_id);

                    let mut locked_rooms = rooms.lock().await;
                    locked_rooms
                        .entry(new_room_id)
                        .or_insert_with(Vec::new)
                        .push(Arc::clone(&writer_arc));

                    let mut writer_locked = writer_arc.lock().await;
                    writer_locked
                        .write_all(format!("You created room {}\n", new_room_id).as_bytes())
                        .await
                        .unwrap();
                } else if let Ok(num) = res.parse::<u32>() {
                    let mut locked_rooms = rooms.lock().await;

                    if let Some(room_writers) = locked_rooms.get_mut(&num) {
                        room_writers.push(Arc::clone(&writer_arc));

                        let mut writer_locked = writer_arc.lock().await;
                        writer_locked
                            .write_all(format!("Joined room {}\n", num).as_bytes())
                            .await
                            .unwrap();
                    } else {
                        let mut writer_locked = writer_arc.lock().await;
                        writer_locked
                            .write_all(b"Invalid room number. Try again.\n")
                            .await
                            .unwrap();
                    }
                } else if bytes_read >= 4 {
                    // Extract room ID from first 4 bytes
                    let s = str::from_utf8(&buf[0..4]).unwrap();
                    let room_id: u32 = s.parse().unwrap();
                    println!("{:?}", room_id);
                    let message = &buf[4..bytes_read]; // The actual message

                    let locked_rooms = rooms.lock().await;

                    if let Some(room_writers) = locked_rooms.get(&room_id) {
                        for w in room_writers {
                            let mut w_locked = w.lock().await;
                            w_locked.write_all(message).await.unwrap();
                        }
                    } else {
                        let mut writer_locked = writer_arc.lock().await;
                        writer_locked
                            .write_all(b"Invalid room ID in message.\n")
                            .await
                            .unwrap();
                    }
                }
            }
        });
    }
}
