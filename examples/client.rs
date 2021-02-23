use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::net::{TcpStream, TcpListener};

use std::error::Error;
use std::io::{self, Write}; // Use the tokio variant later
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
// use std::cell::Cell; Perhaps use it instead of just reasigning the username.

use chat_server::{Serialize, Deserialize, Packet};
use chat_server::request::Command; 
use chat_server::respond::Response;


type Users    = HashMap<String, TcpStream>;
type Messages = Arc<Mutex<HashMap<String, Vec<String>>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("connecting to host");
    let mut stream = TcpStream::connect("127.0.0.1:6142").await?;
    println!("connected");

    let mut users: Users    = HashMap::new();
    let messages:  Messages = Arc::new(Mutex::new(HashMap::new()));
    let mut username: Option<String> = None;
    loop {

        stream.writable().await?;
        let messages = messages.clone();

        let command = command_from_stdin();
          
        // If the command is a message, check and see if we already have a connection with the end user of the message
        // if not, later on we ask the server for the ip and start the connection.
        if let Command::Message(ref name, ref msg) = command {
            match users.get_mut(name) {
                Some(stream) => {
                    if let None = username { continue; }
                    else if let Some(ref username) = username {
                        let cmd = Command::Message(username.to_string(), msg.to_string()).serialize(); // Replace "me" with name of user
                        stream.write_all(&Packet::to_byte_vec(cmd)).await?;
                        stream.write_all(&[0, 0, 0, 0]).await?;  // Unessecary extra sys call
                        continue;
                    }
                },
                None => (),
            }
        }

        if let Command::Show = command {
            let mut messages = messages.lock().unwrap();
            for (name, msg) in messages.drain() {
                println!("{}: ", name);
                msg.iter().for_each(|m| println!("{}", m));
                println!("-------------------");
            }
            continue;
        }

        // send request
        let message = Packet::to_byte_vec(command.serialize());
        stream.write_all(&message).await?; // Perhaps replace with try_write
        stream.write_all(&[0, 0, 0, 0]).await?; // Unessecary extra sys call 
        if let Command::Exit = command {
            break;
        }

        // get respond
        let data = response(&mut stream).await?;
        let data: Option<Response> = data.deserialize();

        match data {
            None => (),
            Some(Response::Login(name, addr))  => {
                println!("Logged in as {}", name);
                println!("At {}", addr);
                username = Some(name);

                // Start listening for connections
                let listener = TcpListener::bind(addr).await.unwrap();
                tokio::spawn(async move {
                    loop {
                        let (socket, _) = listener.accept().await.unwrap();
                        let messages = messages.clone();
                        tokio::spawn(async move {
                            match process_socket(socket, messages).await {
                                Err(e) => println!("{:?}", e),
                                Ok(()) => (),
                            }
                        });
                    }
                });
            },
            Some(Response::Logout) => println!("Logged out"),
            Some(Response::Exit)   => println!("You exited"), // Will not show
            Some(Response::Search(users)) => {
                // Prints the users name and address
                println!("-------------------");
                users.iter()
                    .for_each(|(name, addr)| {
                        println!("name: {}", name);
                        println!("Address: {}", addr);
                        println!("-------------------");
                    })
            },
            Some(Response::Message(name, msg, addr)) => {
                let mut stream = TcpStream::connect(addr).await?;
                stream.writable().await?;

                if let None = username { continue; }
                else if let Some(ref username) = username {
                    let cmd = Command::Message(username.to_string(), msg).serialize();
                    stream.write_all(&Packet::to_byte_vec(cmd)).await?;
                    stream.write_all(&[0, 0, 0, 0]).await?;  // Unessecary extra sys call
                    users.insert(name, stream);
                }
            }
        }
    }
    Ok(())
}

async fn process_socket(mut socket: TcpStream, messages: Messages) -> Result<(), Box<dyn Error>> {
    loop {
        let bytes = request(&mut socket).await?;
        let command = bytes.deserialize().unwrap();
        match command {
            Command::Message(name, msg) => {
                let mut messages = messages.lock().unwrap();
                match messages.get_mut(&name) {
                    Some(user) => user.push(msg),
                    None       => {
                        messages.insert(name, vec![msg]);
                    },
                }
            },
            _ => ()
        }
    }
}

// NOTE: DRY code!!
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

// NOTE: DRY CODE!!
fn get_buffer(amount: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(amount as usize);
    for _ in 0..amount {
        buf.push(0);
    }
    buf
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
            let ip:Vec<Option<u8>> = string.next()?
                .split('.').map(|x| u8::from_str_radix(x, 10).ok()).collect();
            let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(ip[0]?, ip[1]?, ip[2]?, ip[3]?)), 8080);
            Some(Command::Login(nickname, addr))
        },
        Some("logout") => Some(Command::Logout),
        Some("exit")   => Some(Command::Exit),
        Some("search") => Some(Command::Search(String::from(string
                                                            .next()
                                                            .unwrap_or("")))),
        Some("msg")    => {
            let name = string.next()?.to_string();
            let msg = string.fold("".to_string(), |acc, s| acc + s + " ").trim().to_string();
            Some(Command::Message(name, msg))
        },
        Some("show")   => Some(Command::Show),
        _              => None,
    }
}

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
  
