//use hex::encode;
use serde_json::{self, Map};
use std::{collections::HashMap, env};

// Available if you need it!
// use serde_bencode

fn extract_string(encoded_value: &str) -> (Option<serde_json::Value>, &str) {
    //println!("processing string {} ", encoded_value);
    // Example: "5:hello" -> "hello"
    let colon_index = encoded_value.find(':').unwrap();
    let number_string = &encoded_value[..colon_index];
    let number = number_string.parse::<i64>().unwrap();
    let end_point = colon_index + 1 + number as usize;
    let string = &encoded_value[colon_index + 1..end_point];
    return (
        Some(serde_json::Value::String(string.to_string())),
        &encoded_value[end_point..],
    );
}

fn extract_number(encoded_value: &str) -> (Option<serde_json::Value>, &str) {
    //println!("processing number {} ", encoded_value);
    if let Some(e_index) = encoded_value.find('e') {
        let number_string = &encoded_value[1..e_index];
        if let Ok(number) = number_string.parse() {
            return (
                Some(serde_json::Value::Number(number)),
                &encoded_value[e_index + 1..],
            );
        }
    }
    panic!("encoded integer invalid ({encoded_value})")
}

fn extract_list(encoded_value: &str) -> (Option<serde_json::Value>, &str) {
    // locate the start and end of the list.
    //println!("processing list {} ", encoded_value);
    if let Some(start_index) = encoded_value.find('l') {
        //we recurse to get the values from the list
        //calls know what they take and return unprocessed
        //elements
        let mut current_str = &encoded_value[start_index + 1..];
        //println!("begin processing elements  {} ", current_str);
        let mut vec = Vec::new();
        let mut keep_processing = true;
        while keep_processing {
            match decode_bencoded_value_r(&current_str) {
                (Some(v), remaining) => {
                    //add element into vector for list.
                    //println!(
                    //     "pushing {} , remaining: '{}'[{}]",
                    //     v.to_string(),
                    //     remaining,
                    //     remaining.len()
                    // );
                    vec.push(v);
                    current_str = remaining
                }
                (None, remaining) => {
                    keep_processing = false;
                    current_str = remaining;
                }
            }
        }
        return (Some(serde_json::Value::Array(vec)), current_str);
    }
    panic!("encoded list invalid ({encoded_value})")
}

fn extract_dictionary(encoded_value: &str) -> (Option<serde_json::Value>, &str) {
    if let Some(start_index) = encoded_value.find('d') {
        //similarly to list we need to recurse, however
        //the format differs in that we MUST have a string then colon
        //then recurse - skip past the 'd'
        let mut current_str = &encoded_value[start_index + 1..];
        //println!("begin processing map elements  {} ", current_str);
        let mut map = Map::new();
        //again we will try toprocess an 'e' when we reach the end of the
        //map (like list), this returns None and the remaining string
        let mut keep_processing = true;
        while keep_processing {
            if current_str.chars().next().unwrap() != 'e' {
                match extract_string(current_str) {
                    (Some(key), remainder) => {
                        //println!("key  {} / remainder '{}' ", key.to_string(), remainder);
                        match decode_bencoded_value_r(&remainder) {
                            (Some(v), remaining) => {
                                //add element into vector for list.
                                // println!(
                                //     "adding {}:{} , remaining: '{}'[{}]",
                                //     key.to_string(),
                                //     v.to_string(),
                                //     remaining,
                                //     remaining.len()
                                // );
                                map.insert(key.to_string(), v);
                                current_str = remaining
                            }
                            (None, remaining) => {
                                keep_processing = false;
                                current_str = remaining;
                            }
                        }
                    }
                    (None, _) => panic!("Invalid map format {}", encoded_value),
                }
                //println!("remaining '{}' ", current_str);
            }
            else {
                keep_processing = false;
                current_str = &current_str[1..];
            }
        }
        return (Some(serde_json::Value::Object(map)), &current_str);
    }
    panic!("encoded dictionary invalid ({encoded_value})")
}

fn container_end(encoded_value: &str) -> (Option<serde_json::Value>, &str) {
    // we have the end of the list so we need to indicate that
    (None, &encoded_value[1..])
}

#[allow(dead_code)]
fn decode_bencoded_value_r(encoded_value: &str) -> (Option<serde_json::Value>, &str) {
    //
    // This is a recursive call so I need to
    //
    if encoded_value.chars().next().unwrap().is_digit(10) {
        return extract_string(encoded_value);
    } else if encoded_value.chars().next().unwrap() == 'i' {
        return extract_number(encoded_value);
    } else if encoded_value.chars().next().unwrap() == 'l' {
        return extract_list(encoded_value);
    } else if encoded_value.chars().next().unwrap() == 'd' {
        return extract_dictionary(encoded_value);
    } else if encoded_value.chars().next().unwrap() == 'e' {
        return container_end(encoded_value);
    } else {
        panic!("Unhandled encoded value: {}", encoded_value)
    }
}

fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    // basically we have one item being encoded in the example so
    // at the top level I can return just the value
    match decode_bencoded_value_r(encoded_value) {
        (Some(v), _) => v,
        (None, _) => panic!("Got none why!!!"),
    }
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value.to_string());
    } else {
        println!("unknown command: {}", args[1])
    }
}
