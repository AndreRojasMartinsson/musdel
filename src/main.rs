use std::env;
use std::io::Result;

use bytemuck::checked::from_bytes;
use bytemuck::{bytes_of, Pod, Zeroable};
use device_query::{DeviceQuery, DeviceState, MouseState};
use enigo::{Enigo, Mouse, Settings};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[repr(C)] // Ensure the struct has a consistent memory layout
#[derive(Copy, Clone, Pod, Zeroable)]
struct MouseEvent {
    x: i32,
    y: i32,
}

async fn server() -> Result<()> {
    let listener = TcpListener::bind("0.0.0.0:8005").await?;
    println!("Listening on port 8005...");

    let (mut socket, addr) = listener.accept().await?;
    println!("Connected to {:?}", addr);

    let settings = Settings::default();
    let mut enigo = Enigo::new(&settings).unwrap();

    let mut buffer = [0u8; std::mem::size_of::<MouseEvent>()];
    while let Ok(size) = socket.read(&mut buffer).await {
        if size == 0 {
            break;
        }

        let event: &MouseEvent = from_bytes(&buffer);
        println!("Recieved MouseEvent: x = {}, y = {}", event.x, event.y);
        enigo
            .move_mouse(event.x, event.y, enigo::Coordinate::Abs)
            .unwrap();
    }

    Ok(())
}

async fn client(server_ip: Option<String>) -> Result<()> {
    let server_ip = server_ip.unwrap_or("0.0.0.0".to_string());
    let mut socket = TcpStream::connect(format!("{}:8005", server_ip)).await?;
    println!("Connected to server: {}:8005", server_ip);

    let device_state = DeviceState::new();
    let mut last_position = (0, 0);

    loop {
        let mouse: MouseState = device_state.get_mouse();
        let current_position = (mouse.coords.0, mouse.coords.1);

        if current_position != last_position {
            let event = MouseEvent {
                x: current_position.0,
                y: current_position.1,
            };

            let bytes = bytes_of(&event);
            socket.write_all(bytes).await?;
            last_position = current_position;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let command = env::args().nth(1);
    let server_ip = env::args().nth(2);

    if let Some(command) = command {
        match command.as_ref() {
            "client" => client(server_ip).await?,
            "server" => server().await?,
            _ => panic!("Incorrect command. Either client, or server is valid."),
        }
    }

    Ok(())
}
