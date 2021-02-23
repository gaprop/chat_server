use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
// use crate::request::Command;

type Msg = String;
type Nickname = String;

pub mod request {
    use std::net::SocketAddr;
    use crate::{Serialize, Deserialize, Packet, to_packet, Nickname, Msg};

    #[derive(Debug)]
    pub enum Command {
        Login(Nickname, SocketAddr),
        Logout,
        Search(Nickname),
        Exit,
        Message(Nickname, Msg),
        Show,
    }

    impl Serialize for Command {
        fn serialize(&self) -> Vec<Packet> {
            match &self {
                Command::Login(name, addr) => {
                    let mut packet = to_packet(name.bytes().len(), 0);
                    for byte in name.bytes() {
                        packet.data.push(byte);
                    }
                    // packet.data.append(&mut addr.serialize());
                    vec![packet, addr.serialize().pop().unwrap()]
                },
                Command::Logout => vec![to_packet(0, 1)],
                Command::Search(string) => {
                    let mut packet = to_packet(string.bytes().len(), 2);
                    for byte in string.bytes() {
                        packet.data.push(byte);
                    }
                    vec![packet]
                },
                Command::Exit => vec![to_packet(0, 3)],
                Command::Message(name, msg) => {
                    let mut name_packet = to_packet(name.bytes().len(), 4);
                    for byte in name.bytes() {
                        name_packet.data.push(byte);
                    }

                    let mut msg_packet = to_packet(msg.bytes().len(), 4);
                    for byte in msg.bytes() {
                        msg_packet.data.push(byte);
                    }

                    vec![name_packet, msg_packet]
                },
                Command::Show => vec![to_packet(0, 5)],
            }
        }
    }


    impl Deserialize<Command> for Vec<Packet> {
        fn deserialize(&self) -> Option<Command> {
            let packets = &mut self.iter();
            let packet = packets.next()?;

            match packet.data_type {
                0 => {
                    let name = String::from_utf8(packet.data.to_vec()).ok()?;
                    let addr = packets.next()?.deserialize()?;
                    Some(Command::Login(name, addr))
                },
                1 => Some(Command::Logout),
                2 => {
                    let name = String::from_utf8(packet.data.to_vec()).ok()?;
                    Some(Command::Search(name))
                },
                3 => Some(Command::Exit),
                4 => {
                    let name = String::from_utf8(packet.data.to_vec()).ok()?;
                    let msg = String::from_utf8(packets.next()?.data.to_vec()).ok()?;
                    Some(Command::Message(name, msg))
                },
                5 => Some(Command::Show),
                _ => None,
            }
        }
    }



}

pub mod respond {
    use std::net::SocketAddr;
    use crate::{Serialize, Deserialize, Packet, to_packet, Nickname, Msg};

    #[derive(Debug)]
    pub enum Response {
        Login(Nickname, SocketAddr),
        Search(Vec<(Nickname, SocketAddr)>),
        Logout,
        Exit,
        Message(Nickname, Msg, SocketAddr),
    }

    impl Serialize for Response {
        fn serialize(&self) -> Vec<Packet> {
            match self {
                Response::Login(name, addr) => {
                    let mut packet = to_packet(name.bytes().len(), 0);
                    for byte in name.bytes() {
                        packet.data.push(byte);
                    }
                    vec![packet, addr.serialize().pop().unwrap()]
                },
                Response::Search(users) => users.iter()
                    .flat_map(|(name, addr)| {
                        let name: Vec<u8> = name.bytes().collect();
                        let mut addr = addr.serialize();
                        let name_packet = Packet { amount: name.len() as u32 + 1, data_type: 1, data: name};
                        let mut data = vec![name_packet];
                        data.append(&mut addr);
                        data
                    })
                    .collect(),
                Response::Logout => vec![Packet {
                    amount: 1,
                    data_type: 2,
                    data: Vec::new(),
                }],
                Response::Exit => vec![Packet {
                    amount: 1,
                    data_type: 3,
                    data: Vec::new(),
                }],
                Response::Message(name, msg, addr) => {
                    let mut name_packet = to_packet(name.bytes().len(), 4);
                    for byte in name.bytes() {
                        name_packet.data.push(byte);
                    }

                    let mut msg_packet = to_packet(msg.bytes().len(), 4);
                    for byte in msg.bytes() {
                        msg_packet.data.push(byte);
                    }

                    vec![name_packet, msg_packet, addr.serialize().pop().unwrap()]
                }
            }
        }
    }

    impl Serialize for Option<Response> {
        fn serialize(&self) -> Vec<Packet> {
            match self {
                Some(res) => res.serialize(),
                None      => vec![Packet { amount: 0, data_type: 0, data: Vec::new() }],
            }
        }
    }

    impl Deserialize<Response> for Vec<Packet> {
        fn deserialize(&self) -> Option<Response> {
            let packet = self.get(0)?;
            match packet.data_type {
                0 => {
                    let mut packets = self.iter();
                    packets.next();
                    let name = String::from_utf8(packet.data.to_vec()).ok()?;
                    let addr = packets.next()?.deserialize()?;
                    Some(Response::Login(name, addr))
                },
                1 => {
                    let mut users = Vec::with_capacity(self.len());
                    let mut packets = self.iter();
                    while let Some(user) = packets.next() {
                        let name = String::from_utf8(user.data.to_vec()).ok()?;
                        let addr = packets.next()?.deserialize()?;
                        users.push((name, addr));
                    }
                    Some(Response::Search(users))
                },
                2 => Some(Response::Logout),
                3 => Some(Response::Exit),
                4 => {
                    let name = String::from_utf8(self.get(0)?.data.to_vec()).ok()?;
                    let msg = String::from_utf8(self.get(1)?.data.to_vec()).ok()?;
                    let addr = self.get(2)?.deserialize()?;
                    Some(Response::Message(name, msg, addr))
                }
                _ => None,
            }
        }
    }
}

#[derive(Debug)]
pub struct Packet {
    pub amount: u32,
    pub data_type: u8,
    pub data: Vec<u8>,
}

impl Packet {
    pub fn new(amount: u32, data_type: u8, data: Vec<u8>) -> Packet {
        Packet { amount, data_type, data }
    }
    pub fn to_byte_vec(mut packets: Vec<Packet>) -> Vec<u8> {
        packets.iter_mut()
            .flat_map(|p| {
                if p.amount == 0 { return Vec::new(); }
                let mut bytes = Vec::from(p.amount.to_be_bytes());
                bytes.push(p.data_type);
                bytes.append(&mut p.data);
                bytes
            })
            .collect()
    }
}

// This function is really bad
impl Deserialize<SocketAddr> for Packet {
    fn deserialize(&self) -> Option<SocketAddr> {
        let packet = self;
        match packet.data_type {
            // For ip v4
            0 => {
                let ip = packet.data.to_vec();
                let port = &packet.data;
                let mut ip = ip.iter().take(4);
                let mut port = port.iter().skip(4).take(2);
                let port = u16::from_be_bytes([*port.next()?, *port.next()?]);
                let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(*ip.next()?, 
                                                                    *ip.next()?, 
                                                                    *ip.next()?, 
                                                                    *ip.next()?)), port);
                Some(addr)
            }
            1 => {
                let mut data = packet.data.iter();
                let mut ip = Vec::new();
                for _ in 0..8 {
                    ip.push(u16::from_be_bytes([*data.next()?, *data.next()?]));
                }
                let mut ip = ip.iter();
                let port = u16::from_be_bytes([*data.next()?, *data.next()?]);
                let addr = SocketAddr::new(IpAddr::V6(
                        Ipv6Addr::new(*ip.next()?, 
                                      *ip.next()?, 
                                      *ip.next()?, 
                                      *ip.next()?, 
                                      *ip.next()?, 
                                      *ip.next()?, 
                                      *ip.next()?, 
                                      *ip.next()?)), port);
                Some(addr)
            }
            _ => None
        }
    }
}

// Remove magic number
impl Serialize for SocketAddr {
    fn serialize(&self) -> Vec<Packet> {
        match &self {
            SocketAddr::V4(addr) => {
                let octets     = addr.ip().octets();
                let port_bytes = addr.port().to_be_bytes();
                let length     = octets.len() + port_bytes.len() + 1; // One more because of addr type
                
                let mut data = Vec::new();
                for byte in octets.iter() {
                    data.push(*byte);
                }
                for byte in port_bytes.iter() {
                    data.push(*byte);
                }

                vec![Packet { amount: length as u32, data_type: 0, data  }]
            },
            SocketAddr::V6(addr) => {
                let octets     = addr.ip().octets();
                let port_bytes = addr.port().to_be_bytes();
                let length     = octets.len() + port_bytes.len() + 1; // One more because of addr type
                
                let mut data = Vec::new();
                for byte in octets.iter() {
                    data.push(*byte);
                }
                for byte in port_bytes.iter() {
                    data.push(*byte);
                }

                vec![Packet { amount: length as u32, data_type: 1, data }]
            },
        }
    }
}

fn to_packet(size: usize, num: u8) -> Packet {
    let size = size + 1;
    Packet { amount: size as u32, data: Vec::new(), data_type: num }
}

pub trait Serialize {
    fn serialize(&self) -> Vec<Packet>;
}

pub trait Deserialize<T> {
    fn deserialize(&self) -> Option<T>;
}

