use tokio::net::TcpStream;
use tokio::io::{AsyncWriteExt, AsyncReadExt};

use std::error::Error;
use std::io::{self, Write}; // Use the tokio variant later
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use echo_server::{Serialize, Deserialize, Packet};
use echo_server::request::Command; 

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("connecting to host");
    let mut stream = TcpStream::connect("127.0.0.1:6142").await?;
    println!("connected");

    loop {

        stream.writable().await?;

        // request
        let command = command_from_stdin();
        let message = Packet::to_byte_vec(command.serialize());
        stream.write_all(&message).await?; // Perhaps replace with try_write
        stream.write_all(&[0, 0, 0, 0]).await?; // Unessecary extra sys call 
        if let Command::Exit = command {
            break;
        }

        // respond
        let data = response(&mut stream).await?;
        println!("{:?}", data);
        let data = data.deserialize();
        println!("{:?}", data);
    }
    Ok(())
}

fn command_from_stdin() -> Command {
    let mut command = None;
    while let None = command {
        prompt();
        let mut msg = String::new();
        io::stdin().read_line(&mut msg).unwrap();

        command = string_to_command(msg);
    }
    command.unwrap()
}

fn prompt() {
    print!("> ");
    io::stdout().flush().unwrap();
}

fn string_to_command(string: String) -> Option<Command> {
    let mut string = string.trim().split(' ');
    let head = string.next();
    match head {
        Some("login")  => {
            let nickname = String::from(string.next().unwrap_or(""));
            let addr     = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
            Some(Command::Login(nickname, addr))
        },
        Some("logout") => Some(Command::Logout),
        Some("exit")   => Some(Command::Exit),
        Some("search") => Some(Command::Search(String::from(string
                                                            .next()
                                                            .unwrap_or("")))),
        _              => None,
    }
}

// async fn request(stream: &TcpStream) -> Result<(), Box<dyn Error>> {
// } 

// NOTE: dry code, is also in main
async fn response(stream: &mut TcpStream) -> Result<Vec<Packet>, String> {
    stream.readable().await.or(Err(format!("can't become ready")))?;
    let mut bytes = Vec::new();
    loop {
        let amount = match stream.read_u32().await {
            Ok(0) => break,
            Ok(n) if n == 0 => break,
            Ok(n) => n,
            // Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break, // wut is this?
            Err(_)     => return Err(format!("error reading size")),
        };
        let mut buffer = get_buffer(amount);
        match stream.read_exact(&mut buffer).await {
            Ok(0)  => break,
            Ok(_)  => {
                let data_type = buffer.remove(0);
                let packet = Packet::new(amount, data_type, buffer);
                bytes.push(packet)
            },
            // Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => continue,
            Err(_) => return Err(format!("error when reading")),
        }
    }
    Ok(bytes)
}
  
// Duplicate code
fn get_buffer(amount: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(amount as usize);
    for _ in 0..amount {
        buf.push(0);
    }
    buf
}
  
