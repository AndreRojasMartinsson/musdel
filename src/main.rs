use std::env;
use std::net::SocketAddr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bytemuck::{bytes_of, from_bytes, pod_read_unaligned, Pod, Zeroable};
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
struct PackedMouseDelta {
    sequence_number: u32,
    timestamp: u32,
    data: u32, // DX and DY Packed into 12 bits each
               //dx: i16, // Relative mouse movement
               //dy: i16,
               //timestamp: u32, // Time of movement (used for latency compensation)
}
impl PackedMouseDelta {
    pub fn new(sequence_number: u32, timestamp: u32, dx: i16, dy: i16) -> Self {
        let packed_dx = (dx & 0xFFF) as u32; // Mask to 12 bits;
        let packed_dy = (dy & 0xFFF) as u32; // Mask to 12 bits;
                                             //
        Self {
            sequence_number,
            timestamp,
            data: (packed_dx << 12) | packed_dy,
        }
    }

    pub fn unpack(&self) -> (i16, i16) {
        let dx = ((self.data >> 12) & 0xFFF) as i16;
        let dy = (self.data & 0xFFF) as i16;

        (dx, dy)
    }
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
    let mut buffer = [0u8; std::mem::size_of::<PackedMouseDelta>()];
    let mut last_delta: Option<(u32, i16, i16)> = None;

    loop {
        if let Ok((size, _)) = socket.recv_from(&mut buffer).await {
            if size == std::mem::size_of::<PackedMouseDelta>() {
                let delta: &PackedMouseDelta = from_bytes(&buffer);
                let (dx, dy) = delta.unpack();
                println!(
                    "Recieved: seq={}, timestamp={}, dx = {}, dy = {}",
                    delta.sequence_number, delta.timestamp, dx, dy
                );

                if let Some((last_timestamp, last_dx, last_dy)) = last_delta {
                    let dt = (delta.timestamp - last_timestamp).max(1) as f32;
                    let steps = 5;

                    for i in 1..=steps {
                        let interp_dx =
                            last_dx as f32 + (dx as f32 - last_dx as f32) * i as f32 / steps as f32;

                        let interp_dy =
                            last_dy as f32 + (dy as f32 - last_dy as f32) * i as f32 / steps as f32;

                        enigo
                            .move_mouse(interp_dx as i32, interp_dy as i32, enigo::Coordinate::Rel)
                            .unwrap();
                        tokio::time::sleep(tokio::time::Duration::from_millis(
                            (dt / steps as f32) as u64,
                        ))
                        .await;
                    }
                } else {
                    enigo
                        .move_mouse(dx as i32, dy as i32, enigo::Coordinate::Rel)
                        .unwrap();
                }

                last_delta = Some((delta.timestamp, dx, dy));
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
    let mut sequence_number = 0u32;

    loop {
        let mouse: MouseState = device_state.get_mouse();
        let current_position = (mouse.coords.0, mouse.coords.1);

        //let dx = current_position.0 - last_position.0;
        //let dy = current_position.1 - last_position.1;

        if current_position != last_position {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u32;
            let delta = PackedMouseDelta::new(
                sequence_number,
                timestamp,
                (current_position.0 - last_position.0) as i16,
                (current_position.1 - last_position.1) as i16,
            );

            socket.send(bytes_of(&delta)).await?;
            last_position = current_position;
            sequence_number = sequence_number.wrapping_add(1);
        }

        //if dx != 0 || dy != 0 {
        //    let event = MouseDelta {
        //        dx: dx as i16,
        //        dy: dy as i16,
        //        timestamp: last_send_time.elapsed().as_millis() as u32,
        //    };
        //
        //    let bytes = bytes_of(&event);
        //    socket.send(bytes).await?;
        //    last_position = current_position;
        //    last_send_time = Instant::now();
        //}

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
