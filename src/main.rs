use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, TcpListener};

use echo_server::{Packet, Deserialize, Serialize};
use echo_server::request::Command;
use echo_server::respond::Response;
// use echo_server::{Packet, Deserialize, Command};

use std::net::SocketAddr;
use std::error::Error;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::cell::Cell;

type Users = Arc<Mutex<HashMap<String, SocketAddr>>>;

#[tokio::main]
async fn main() -> io::Result<()> {
    let users: Users = Arc::new(Mutex::new(HashMap::new()));
    let listener = TcpListener::bind("127.0.0.1:6142").await.unwrap();

    loop {
        let (socket, _) = listener.accept().await?;
        // println!("Client connected from {}", )
        let users = users.clone();

        tokio::spawn(async move {
            match process_socket(socket, users).await {
                Err(e) => println!("{:?}", e),
                Ok(()) => return (),
            }
        });
    }
}

async fn process_socket(mut socket: TcpStream, users: Users) -> Result<(), Box<dyn Error>> {
    let mut curr_user = Cell::new(None);
    loop {
        let bytes = request(&mut socket).await?;
        let command = bytes.deserialize().unwrap(); // NOTE: Handle this
        let res = handle_command(command, &users, &mut curr_user);
        println!("{:?}", res);
        response(&mut socket, res).await?;
    }
}

fn handle_command(command: Command, 
                  users: &Users, 
                  curr_user: &mut Cell<Option<(String, SocketAddr)>>) -> Option<Response> {
    match command {
        Command::Login(name, addr) => {
            if name == "" { return None; }
            let mut users = users.lock().unwrap();
            if users.contains_key(&name) { None }
            else {
                curr_user.set(Some((name.clone(), addr)));
                users.insert(name, addr);
                Some(Response::Login)
            }
        },
        Command::Search(name) => {
            let users = users.lock().unwrap();
            if name == "" { None }
            else if name == "all" {
                let users: Vec<(String, SocketAddr)> = users.keys()
                        .cloned().zip(users.values().cloned()).collect();
                if users.len() == 0 { None }
                else { Some(Response::Search(users)) }
            } else {
                match users.get(&name) {
                    Some(addr) => Some(Response::Search(Vec::from([(name, *addr)]))),
                    None       => None
                }
            }
        },
        Command::Logout => if let Some((name, _)) = curr_user.get_mut() {
            let mut users = users.lock().unwrap();
            users.remove(name);
            Some(Response::Logout)
        } else {
            None
        },
        Command::Exit   => {
            if let Some((name, _)) = curr_user.get_mut() {
                let mut users = users.lock().unwrap();
                users.remove(name);
            }
            Some(Response::Exit)
        },
    }
}

async fn response(socket: &mut TcpStream, res: Option<Response>) -> Result<(), Box<dyn Error>> {
    if let Some(Response::Exit) = res { return Ok(()); }
    socket.writable().await?;
    let bytes = Packet::to_byte_vec(res.serialize());
    socket.write_all(&bytes).await?;
    socket.write_all(&[0, 0, 0, 0]).await?;  // Unessecary extra sys call
    Ok(())
}

async fn request(socket: &mut TcpStream) -> Result<Vec<Packet>, Box<dyn Error>> {
    socket.readable().await?;
    let mut bytes = Vec::new();
    loop {
        let amount = match socket.read_u32().await {
            Ok(0) => break,
            Ok(n) if n == 0 => break,
            Ok(n) => n,
            // Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break, // wut is this?
            Err(e)     => return Err(Box::new(e)),
        };
        let mut buffer = get_buffer(amount);
        match socket.read_exact(&mut buffer).await {
            Ok(0)  => break,
            Ok(_)  => {
                let data_type = buffer.remove(0);
                let packet = Packet::new(amount, data_type, buffer);
                bytes.push(packet)
            },
            // Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(e) => return Err(Box::new(e)),
        }
    }
    Ok(bytes)
}

fn get_buffer(amount: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(amount as usize);
    for _ in 0..amount {
        buf.push(0);
    }
    buf
}
