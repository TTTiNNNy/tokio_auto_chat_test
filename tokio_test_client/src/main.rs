use serde_json::{Value};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::time::sleep;
use std::collections::{VecDeque};
use std::env::{self};
use std::error::Error;
use std::sync::{Arc};
use std::time::Duration;


// make the client a bot with 3 messages
const MESSAGES: [&str; 3] = ["hello", "hi", "yo"];
const INPUT_BUF_LEN: usize = 256;
const SERVER_ADDR: &str = "127.0.0.1:8080";


#[derive(Debug)]
struct UserInfo{
    /// Username
    name: String,
    /// Something like group chat id
    room_id: String,
}

struct Unauth();
struct Auth{
    jwt: String,
    stream: TcpStream
}

struct ApiPath{}

// we make 3 endpoints. 
// Authorization, which receives a jwt token and which is subsequently used by the server 
// when checking and receiving meta information about the client.
// The second is sending a message to the server. 
// The third is receiving all new messages.
impl ApiPath{
    const AUTH: &'static str = "/auth";
    const WRITE: &'static str = "/send";
    const UPDATE: &'static str = "/receive";
}

// since I was too lazy to make data types for json messages, 
// I will use Hashmap structs for work with json during reading and format! macro for writing and remove quotes manually for strings)
fn de_quotes(text: &str) -> String {
    text[1..text.len() - 1].to_string()   
}

impl Unauth{
    async fn auth(&mut self, mut stream: TcpStream, user_info: UserInfo) -> Result<Auth, Box<dyn Error>>{
        let j: String = format!("{{\"path\":\"{}\",\"name\":\"{}\",\"room_id\":\"{}\"}} ", ApiPath::AUTH, user_info.name, user_info.room_id);
        stream.write_all((j).as_bytes()).await?;
        let mut buf: [u8; INPUT_BUF_LEN] = [0; INPUT_BUF_LEN];

        let n = match stream.read(buf.as_mut_slice()).await {
            Ok(n) => {  
                  let s = String::from_utf8_lossy(&buf[0..n]);                  
                  let v: Value = serde_json::from_str(s.as_ref()).unwrap();
              return Ok(Auth{jwt: v["jwt"].to_string(), stream});
            },
            Err(e) => {
                eprintln!("failed to auth socket; err = {:?}", e);
                return Result::Err(Box::new(e));
                
            }
        };
    }
}

impl Auth{
    async fn send(&mut self, message: & String) -> Result<(), Box<dyn Error>>{
        let j = format!("{{\"path\":\"{}\",\"jwt\":{},\"message\":\"{}\"}} ", ApiPath::WRITE,self.jwt, message);
        self.stream.write_all((j).as_bytes()).await?;
        Ok(())
    }
    
    async fn update(&mut self, last_msg_i: &mut usize) ->  Result<String, Box<dyn Error>> {
        let j = format!("{{\"path\":\"{}\",\"jwt\":{},\"last_msg_i\":{last_msg_i}}} ", ApiPath::UPDATE, self.jwt);
        self.stream.write_all((j).as_bytes()).await?;

        let mut buf = [0; INPUT_BUF_LEN];

        let n = match self.stream.read(&mut buf).await {
            Ok(n) => {n},
            Err(e) => {
                eprintln!("failed to read from socket; err = {:?}", e);
                0
            }
        };

        let s = String::from_utf8_lossy(&buf[0..n]).to_string();
        
        //  lets image that some kinds of erros can't be)
        let v: Value = serde_json::from_str(s.as_ref()).unwrap();
        let err = v["err"].to_string();
        
        if v["err"].to_string() == "null".to_string(){
            v["last_msg_i"].to_string();
            let inpit_last_msg_i = v["last_msg_i"].to_string();
            let inpit_last_msg_i = de_quotes(&inpit_last_msg_i);
            let inpit_last_msg_i = usize::from_str_radix(inpit_last_msg_i.as_str(),10).unwrap();
            *last_msg_i = inpit_last_msg_i;
            let msg =  format!("{}", v["message"].to_string());
            return Ok(msg);
        } else if err.to_string() == "\"empty\"".to_string(){ return Ok(String::new()); }
        panic!();
    }
}

enum ClientTasks{
    ChatUpdate,
    SendMessage
}
struct Client{
    msg_read_i: usize,
    message_iter: std::iter::Cycle<std::slice::Iter<'static, &'static str>>,
    auth_trans: Auth,
    tasks_queue: VecDeque<ClientTasks>
}

impl Client{
    async fn tasks_perform(&mut self){
        //println!("tasks_perform");
        while let Some(task) = self.tasks_queue.pop_front() {
            match task {
                ClientTasks::ChatUpdate => { 
                    let msg = de_quotes(self.auth_trans.update(& mut self.msg_read_i).await.unwrap().as_str());
                    if !msg.is_empty() {
                        msg.split(',').into_iter().for_each(|el|{println!("{}", el)});
                }
                },
                ClientTasks::SendMessage => { self.auth_trans.send(&self.message_iter.next().unwrap().to_string()).await.unwrap(); }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    let it: std::iter::Cycle<std::slice::Iter<&str>> = MESSAGES.iter().cycle();
    let stream: TcpStream = TcpStream::connect(SERVER_ADDR).await?;   

    // pass user info from cli args. for example: 
    let args: Vec<String> = env::args().collect();

    let info: UserInfo = UserInfo{name:args[1].clone(),room_id: args[2].to_owned()};
    let mut unauth_trans = Unauth();
    let auth_trans = unauth_trans.auth(stream, info).await?; 
    
    let client = Arc::new(Mutex::new(Client{msg_read_i: 0,message_iter: it, auth_trans: auth_trans, tasks_queue: Default::default()}));
    let send_client = client.clone();
    let update_client = client.clone();

    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(1)).await;

            let mut lock = send_client.lock().await;
            lock.tasks_queue.push_back(ClientTasks::SendMessage);
            drop(lock);

        }
    });

    tokio::spawn(async move {
        loop {
           sleep(Duration::from_secs(3)).await;

            let mut lock = update_client.lock().await;
            lock.tasks_queue.push_back(ClientTasks::ChatUpdate);
            drop(lock);

        }
    });
    loop {
        let mut lock = client.lock().await;
        lock.tasks_perform().await;
        drop(lock);

        sleep(Duration::from_millis(1000)).await;
}
}