use std::collections::VecDeque;

use std::sync::Arc;

use serde_json::Value;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

type NameType = String;
type MessageType = String;
type RoomIdType = String;


use hmac::{Hmac, Mac};
use jwt::{SignWithKey, VerifyWithKey};
use sha2::Sha256;
use tokio::sync::Mutex;

use std::collections::BTreeMap;

const SECRET: &str = "secret";

#[derive(Default)]
struct Room{
    // it's quite logical that the messages will be stored on the server. 
    // Until the first restart)
    messages:VecDeque<(NameType, MessageType)>,
}

struct ApiPath{}

impl Room{
    fn user_info(jwt: String) -> (NameType, RoomIdType){
        let jwt = de_quotes(&jwt);
        let key: Hmac<Sha256> = Hmac::new_from_slice(SECRET.as_bytes()).unwrap();
        let claims: BTreeMap<String, String> = jwt.verify_with_key(&key).unwrap();
        let (name, r_id) = claims.first_key_value().unwrap();
        (name.to_owned(), r_id.to_owned())
    }
}

impl ApiPath{
    const AUTH: &'static str = "\"/auth\"";
    const SEND: &'static str = "\"/send\"";
    const UPDATE: &'static str = "\"/receive\"";
}

fn auth_handl(name: String, room_id: String, stream: &mut TcpStream ) -> Result<String, Box<dyn std::error::Error>> {
    let key: Hmac<Sha256> = Hmac::new_from_slice(SECRET.as_bytes())?;
    let mut claims = BTreeMap::new();
    claims.insert(name, room_id);
    let token_str = claims.sign_with_key(&key)?;
    Ok(token_str)
}

// Dont remove any previus messages. Maybe another users will need read it. 
fn send_handl(room: &mut Room, msg: String, name: String){
    room.messages.push_back((name, msg));
}

// getting all messages after the last message that was read by the user 
// and gluing this file into one giga message).
// for a buffer overflow does not occur, 
// clients must connect to the server from the very beginning. 
// Otherwise, the message will not fit into their buffer. 
// It would be logical to send not one huge message, but a bunch of small,
// but I'm tired)
fn receive_handl(room: &mut Room, last_msg: usize) -> String {
    let mut msg = String::new();
        for i_msg in last_msg..room.messages.len(){
            let (name, _msg) = & room.messages[i_msg];
            msg += format!("{}: {}, ", name, _msg).as_str();
        };
        msg
}
// make "qwe" into qwe
fn de_quotes(text: &str) -> String {
    text[1..text.len() - 1].to_string()   
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    //create 2 example isolated rooms.
    let rooms =  Arc::new(Mutex::new(BTreeMap::from([ 
        ("1".to_string(), Room{..Default::default()}), 
        ("qwe".to_string(), Room{..Default::default()})])));
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    let q = rooms.lock().await;
    drop(q);

    loop {
        let (mut socket, _) = listener.accept().await?;
        let rooms = rooms.clone();

        // giga buffer shall has enough space, case wew can receive many different requests
        tokio::spawn(async move {
            let mut buf = [0; 1024];

            loop {
                let n = match socket.read(&mut buf).await {
                    Ok(n) if n == 0 => return,
                    Ok(n) => {n},
                    Err(e) => {
                        eprintln!("failed to read from socket; err = {:?}", e);
                        return;
                    }
                };
                let s = String::from_utf8_lossy(&buf[0..n]);
                let mut vv: VecDeque<&str> = s.split(' ').collect();

                //parse another requests that was split by ' ' symbol.
                while let Some(s) = vv.pop_front() {
                    if !s.is_empty(){
                        println!("input message: {}", s);
                        let v: Value = serde_json::from_str(s.as_ref()).unwrap();
        
                        let path = v["path"].to_string();

                        match path.as_str() {
                            ApiPath::AUTH => {
                                let jwt = auth_handl(v["name"].to_string(), v["room_id"].to_string(), &mut socket).unwrap(); 
                                let out_msg = format!("{{\"jwt\":\"{}\"}}", jwt);
                                _ = socket.write_all(&out_msg.as_bytes()).await;
                            }
                            
                            ApiPath::UPDATE => {                        
                                let jwt: String = v["jwt"].to_string();
                                let last_msg_i = usize::from_str_radix(&v["last_msg_i"].to_string(),10).unwrap();
                                
                                let mut rooms = rooms.lock().await;
                                let (.., room_id) = Room::user_info(jwt);
                                
                                let room = rooms.get_mut(de_quotes(&room_id).as_str()).unwrap();
                                let msg = receive_handl(room, last_msg_i);
                                
                                if msg.len() != 0 {
                                    let msg= format!("{{\"message\":\"{}\", \"last_msg_i\":\"{}\"}}", msg, room.messages.len());
                                    socket.write_all(msg.as_bytes()).await.unwrap();
                                } else { 
                                    let msg = "{\"err\":\"empty\"}";
                                    socket.write_all(msg.as_bytes()).await.unwrap();        
                                }; 
                                drop(rooms);
        
                        }
                            ApiPath::SEND => {        
                                let jwt = v["jwt"].to_string();
                                let (name, room_id) = Room::user_info(jwt);
                                let mut rooms = rooms.lock().await;
                                let msg = v["message"].to_string();

                                send_handl(rooms.get_mut(&de_quotes(room_id.as_str())).unwrap(), de_quotes(&msg), de_quotes(&name)); 
                                drop(rooms);
                            }
                            _ => {panic!();}
                        }
                    }

                }
            }
        });
    }
}