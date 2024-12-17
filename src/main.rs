use std::env;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bytemuck::{bytes_of, from_bytes, Pod, Zeroable};
use device_query::{DeviceQuery, DeviceState, MouseState};
use enigo::{Enigo, Mouse, Settings};
use std::io::Result;
use tokio::net::UdpSocket;
use tokio::time::sleep;
//use std::env;
//
//use std::time::Instant;

//use bytemuck::checked::from_bytes;
//use bytemuck::{bytes_of, from_bytes, Pod, Zeroable};
//use device_query::{DeviceQuery, DeviceState, MouseState};
//use enigo::{Enigo, Mouse, Settings};
//use tokio::io::{AsyncReadExt, AsyncWriteExt};
//use tokio::net::{TcpListener, TcpStream, UdpSocket};
//use tokio::time::Instant;

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct MouseDelta {
    dx: i16, // Relative mouse movement
    dy: i16,
    timestamp: u32, // Time of movement (used for latency compensation)
}

const SERVER_PORT: u16 = 8005;
const UPDATE_INTERVAL_MS: u64 = 10;

//#[repr(C)] // Ensure the struct has a consistent memory layout
//#[derive(Copy, Clone, Pod, Zeroable)]
//struct MouseEvent {
//    x: i16,
//    y: i16,
//}

async fn server() -> Result<()> {
    let socket = UdpSocket::bind("0.0.0.0:8005").await?;
    println!("UDP Server Listening on port 8005...");

    //let (mut socket, addr) = listener.accept().await?;
    //println!("Connected to {:?}", addr);

    let mut enigo = Enigo::new(&Settings::default()).unwrap();
    let mut buffer = [0u8; std::mem::size_of::<MouseDelta>()];

    loop {
        if let Ok((size, addr)) = socket.recv_from(&mut buffer).await {
            if size == std::mem::size_of::<MouseDelta>() {
                let event: &MouseDelta = from_bytes(&buffer);
                println!(
                    "Recieved from {}: dx = {}, dy = {}, ts = {}",
                    addr, event.dx, event.dy, event.timestamp
                );

                enigo
                    .move_mouse(event.dx.into(), event.dy.into(), enigo::Coordinate::Rel)
                    .unwrap();
            }
        }
    }

    //while let Ok(size) = socket.read(&mut buffer).await {
    //    if size == 0 {
    //        break;
    //    }
    //
    //    let event: &MouseEvent = from_bytes(&buffer);
    //    println!("Recieved MouseEvent: x = {}, y = {}", event.x, event.y);
    //    enigo
    //        .move_mouse(event.x.into(), event.y.into(), enigo::Coordinate::Abs)
    //        .unwrap();
    //}
    //
    //Ok(())
}

async fn client(server_ip: Option<String>) -> Result<()> {
    let server_addr: SocketAddr = format!(
        "{}:{}",
        server_ip.unwrap_or("127.0.0.1".to_string()),
        SERVER_PORT
    )
    .parse()
    .unwrap();

    let socket = UdpSocket::bind("0.0.0.0:0").await?;
    socket.connect(server_addr).await?;

    println!("Connected to server: {}", server_addr);

    let device_state = DeviceState::new();
    let mut last_position = (0, 0);
    let mut last_send_time = Instant::now();

    loop {
        let mouse: MouseState = device_state.get_mouse();
        let current_position = (mouse.coords.0, mouse.coords.1);

        let dx = current_position.0 - last_position.0;
        let dy = current_position.1 - last_position.1;

        if dx != 0 || dy != 0 {
            let event = MouseDelta {
                dx: dx as i16,
                dy: dy as i16,
                timestamp: last_send_time.elapsed().as_millis() as u32,
            };

            let bytes = bytes_of(&event);
            socket.send(bytes).await?;
            last_position = current_position;
            last_send_time = Instant::now();
        }

        sleep(Duration::from_millis(UPDATE_INTERVAL_MS)).await;
    }

    //let server_ip = server_ip.unwrap_or("0.0.0.0".to_string());
    //let mut socket = TcpStream::connect(format!("{}:8005", server_ip)).await?;
    //println!("Connected to server: {}:8005", server_ip);
    //
    //let device_state = DeviceState::new();
    //let mut last_position = (0, 0);
    //
    //loop {
    //    let mouse: MouseState = device_state.get_mouse();
    //    let current_position = (mouse.coords.0, mouse.coords.1);
    //
    //    if current_position != last_position {
    //        let event = MouseEvent {
    //            x: current_position.0 as i16,
    //            y: current_position.1 as i16,
    //        };
    //
    //        let bytes = bytes_of(&event);
    //        socket.write_all(bytes).await?;
    //        last_position = current_position;
    //    }
    //
    //    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    //}
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
