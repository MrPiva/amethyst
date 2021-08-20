use crate::game::packets::Packet;
use json::JsonValue;
use json::number::Number;
use crate::game::chat::ChatComponent;
use openssl::rsa::{Rsa, Padding};
use openssl::pkey::Private;
use rand::{Rng, thread_rng};
use cfb8::Cfb8;
use aes::Aes128;
use uuid::Uuid;
use aes::cipher::NewStreamCipher;
use regex::Regex;
use rustc_serialize::hex::ToHex;
use openssl::sha::Sha1;
use std::str::FromStr;
use crate::net::newer_network_manager::{RawPacket, PlayerLoginClient, ConnectionState};
use crate::data_reader::DataReader;

pub fn handle(packets: Vec<RawPacket>, client: &mut PlayerLoginClient) {
    for raw in packets {
        let packet = match Packet::read(raw.id, &mut DataReader::new(raw.data), client.state) {Some(t) => t, None => continue};
        match packet {
            Packet::Handshake {next_state, protocol_version, server_address, server_port} => {
                match next_state {
                    1 => client.state = ConnectionState::Status,
                    2 => client.state = ConnectionState::Login,
                    _ => {
                        //TODO DC for unknown handshake next state
                    }
                }
            }
            Packet::StatusRequest => {
                let mut json = JsonValue::new_object();
                let mut version = JsonValue::new_object();
                version["name"] = JsonValue::String("1.8.9".to_owned());
                version["protocol"] = JsonValue::Number(Number::from(47 as u8));
                json["version"] = version;
                let mut players = JsonValue::new_object();
                players["max"] = JsonValue::Number(Number::from(10 as u8));
                players["online"] = JsonValue::Number(Number::from(0 as u8));
                json["players"] = players;
                json["description"] = ChatComponent::new_text("Amethyst Minecraft Server".to_owned()).to_json();
                client.write(Packet::StatusResponse {json});
            }
            Packet::Ping {ping} => client.write(Packet::Pong {pong: ping}),
            Packet::LoginStart {nickname} => {
                client.verify_token = Some(thread_rng().gen::<[u8; 4]>());
                client.write(Packet::EncryptionRequest {server: String::new(), public_key: get_publick_key().clone(), verify_token: client.verify_token.unwrap().clone()});
                client.connection.identifier = nickname.clone();
                client.nickname = Some(nickname)
            }
            Packet::EncryptionResponse {verify_token, shared_secret} => {
                let rsa = get_rsa();
                let mut decrypted_verify_token = [0 as u8; 128];
                match rsa.private_decrypt(&verify_token, &mut decrypted_verify_token, Padding::PKCS1) {
                    Ok(_t) => {},
                    Err(_e) => {
                        //TODO DC for invalid verify token
                    }
                };

                if !decrypted_verify_token[0..4].eq(&client.verify_token.unwrap()) {
                    //TODO DC for wrong verify token
                }

                let mut decrypted_shared_secret = [0 as u8; 128];
                let shared_secret_length = match rsa.private_decrypt(&shared_secret, &mut decrypted_shared_secret, Padding::PKCS1) {
                    Ok(t) => t,
                    Err(_e) => {
                        //TODO DC for invalid shared secret
                        continue;
                    }
                };
                let shared_secret = &decrypted_shared_secret[0..shared_secret_length];

                client.encode = Some(Cfb8::<Aes128>::new_var(shared_secret, shared_secret).unwrap());
                client.decode = Some(Cfb8::<Aes128>::new_var(shared_secret, shared_secret).unwrap());

                let mut sha1 = Sha1::new();
                sha1.update(b"");
                sha1.update(&shared_secret);
                sha1.update(&rsa.public_key_to_der().unwrap());

                client.write(Packet::LoginSuccess {
                    uuid: Uuid::default(),
                    nickname: "britney bitch".to_string()
                });
            }
            _ => {
                //TODO DC for unknown login packet
            }
        }
    }
}

// pub fn handle<'a>(packet: Packet, client: &mut LoggingInClient) -> HandleResult<'a> {
//     return match packet {
//         Packet::Handshake { protocol_version: _, server_address: _, server_port: _, next_state } => {
//             client.state = match next_state {
//                 1 => ConnectionState::Status,
//                 2 => ConnectionState::Login,
//                 _ => return HandleResult::Disconnect("Unknown state on handshake")
//             };
//             HandleResult::Nothing
//         }
//         Packet::StatusRequest => {
//             let mut json = JsonValue::new_object();
//             let mut version = JsonValue::new_object();
//             version["name"] = JsonValue::String("1.8.9".to_owned());
//             version["protocol"] = JsonValue::Number(Number::from(47 as u8));
//             json["version"] = version;
//             let mut players = JsonValue::new_object();
//             players["max"] = JsonValue::Number(Number::from(10 as u8));
//             players["online"] = JsonValue::Number(Number::from(0 as u8));
//             json["players"] = players;
//             json["description"] = ChatComponent::new_text("Amethyst Minecraft Server".to_owned()).to_json();
//             HandleResult::SendPacket(Packet::StatusResponse { json })
//         }
//         Packet::Ping { ping } => HandleResult::SendPacket(Packet::Pong { pong: ping }),
//         Packet::LoginStart { nickname } => {
//             client.nickname = Some(nickname);
//             let public_key = get_rsa().public_key_to_der().unwrap();
//             let verify_token = rand::thread_rng().gen::<[u8; 4]>();
//
//             client.verify_token = Some(verify_token);
//             HandleResult::SendPacket(Packet::EncryptionRequest {
//                 server: String::new(),
//                 public_key_length: public_key.len() as i32,
//                 public_key,
//                 verify_token_length: 4,
//                 verify_token
//             })
//         }
//         Packet::EncryptionResponse { shared_secret_length: _, shared_secret, verify_token_length: _, verify_token } => {
//             let rsa = get_rsa();
//             let mut decrypted_verify_token = [0 as u8; 128];
//             match rsa.private_decrypt(&verify_token, &mut decrypted_verify_token, Padding::PKCS1) {
//                 Ok(_t) => {},
//                 Err(_e) => return HandleResult::Disconnect("Couldn't decrypt verify token")
//             };
//
//             if !decrypted_verify_token[0..4].eq(&client.verify_token.unwrap()) {
//                 return HandleResult::Disconnect("Verify token isn't the same")
//             }
//
//             let mut decrypted_shared_secret = [0 as u8; 128];
//             let shared_secret_length = match rsa.private_decrypt(&shared_secret, &mut decrypted_shared_secret, Padding::PKCS1) {
//                 Ok(t) => t,
//                 Err(_e) => {
//                     return HandleResult::Disconnect("Couldn't decrypt shared secret")
//                 }
//             };
//             let shared_secret = &decrypted_shared_secret[0..shared_secret_length];
//
//             client.encode = Some(Cfb8::<Aes128>::new_var(shared_secret, shared_secret).unwrap());
//             client.decode = Some(Cfb8::<Aes128>::new_var(shared_secret, shared_secret).unwrap());
//
//             let mut sha1 = Sha1::new();
//             sha1.update(b"");
//             sha1.update(&shared_secret);
//             sha1.update(&rsa.public_key_to_der().unwrap());
//
//             // let response = match reqwest::blocking::Client::new().get(&format!("https://sessionserver.mojang.com/session/minecraft/hasJoined?username={}&serverId={}", client.nickname.as_ref().unwrap(), hex_digest(sha1)))
//             //     .send() {
//             //     Ok(ok) => ok,
//             //     Err(e) => {
//             //         println!("Error while contacting sessionserver.mojang.com to login a player: {}, {}", client.nickname.as_ref().unwrap(), e);
//             //         return HandleResult::Disconnect("Couldn't decrypt shared secret")
//             //     }
//             // };
//             // let response_code = response.status().as_u16();
//             // if response_code == 204 {
//             //     return HandleResult::Disconnect("Client not authenticated.")
//             // }
//             // let json = match response.text() {
//             //     Ok(ok) => {
//             //         match json::parse(&ok) {
//             //             Ok(ok) => ok,
//             //             Err(e) => {
//             //                 println!("Error while parsing login response to json: {}, {}", client.nickname.as_ref().unwrap(), e);
//             //                 return HandleResult::Disconnect("An error occured while contacting Mojang.")
//             //             }
//             //         }
//             //     },
//             //     Err(e) => {
//             //         println!("Error while parsing login response to text: {}, {}", client.nickname.as_ref().unwrap(), e);
//             //         return HandleResult::Disconnect("An error occured while contacting Mojang.")
//             //     }
//             // };
//             //
//             // let data = match parse_json(json) {
//             //     Some(t) => t,
//             //     None => {
//             //         println!("Error while parsing login response data to text: {}", client.nickname.as_ref().unwrap());
//             //         return HandleResult::Disconnect("An error occured while contacting Mojang.")
//             //     }
//             // };
//
//             //TODO Arrumar login
//             HandleResult::SendPacket(Packet::LoginSuccess {
//                 uuid: Uuid::default(),
//                 nickname: "".to_string()
//             })
//         }
//         _ => HandleResult::Nothing
//     }
// }

fn parse_json(mut json: JsonValue) -> Option<(Uuid, String)> {
    let uuid = match json["id"].as_str() {
        Some(t) => t,
        None => return None
    };
    let uuid = match Uuid::from_str(uuid) {
        Ok(t) => t,
        Err(e) => return None
    };

    let name = match json["name"].take_string() {
        Some(t) => t,
        None => return None
    };

    return Some((uuid, name));
}

pub static mut RSA: Option<Rsa<Private>> = None;
pub static mut PUBLIC_KEY: Option<Vec<u8>> = None;

#[inline]
fn get_rsa() -> &'static Rsa<Private> {
    unsafe {
        return RSA.as_ref().unwrap();
    }
}

#[inline]
fn get_publick_key() -> &'static Vec<u8> {
    unsafe {
        return PUBLIC_KEY.as_ref().unwrap();
    }
}

fn hex_digest(sha1: Sha1) -> String {
    let mut hex = sha1.finish();

    let negative = (hex[0] & 0x80) == 0x80;

    let regex = Regex::new(r#"^0+"#).unwrap();

    if negative {
        two_complement(&mut hex);
        format!("-{}", regex.replace(&hex.to_hex(), "").to_string())
    }
    else {
        regex.replace(&hex.to_hex(), "").to_string()
    }
}

fn two_complement(bytes: &mut [u8; 20]) {
    let mut carry = true;
    for i in (0..bytes.len()).rev() {
        bytes[i] = !bytes[i] & 0xff;
        if carry {
            carry = bytes[i] == 0xff;
            bytes[i] = bytes[i] + 1;
        }
    }
}