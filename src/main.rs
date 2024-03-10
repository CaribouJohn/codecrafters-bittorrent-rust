//use hex::encode;
use serde_json;
use std::env;

// Available if you need it!
// use serde_bencode

fn extract_string(encoded_value: &str) -> (serde_json::Value,&str) {
    // Example: "5:hello" -> "hello"
    let colon_index = encoded_value.find(':').unwrap();
    let number_string = &encoded_value[..colon_index];
    let number = number_string.parse::<i64>().unwrap();
    let end_point = colon_index + 1 + number as usize;
    let string = &encoded_value[colon_index + 1 .. end_point];
    return (serde_json::Value::String(string.to_string()),&encoded_value[end_point..]);
}

fn extract_number( encoded_value: &str) -> (serde_json::Value,&str) {
    if let Some(e_index) = encoded_value.find('e') {
        let number_string = &encoded_value[1..e_index];
        if let Ok(number) = number_string.parse() {
            return (serde_json::Value::Number(number),&encoded_value[e_index+1..]);
        }
    }
    panic!("encoded integer invalid ({encoded_value})")
}

#[allow(dead_code)]
fn decode_bencoded_value_r(encoded_value: &str) -> (serde_json::Value,&str) {
    // If encoded_value starts with a digit, it's a number

    // the list / array starts with l
    if encoded_value.chars().next().unwrap() == 'l' {
        //we need to call this function until we get an error
        let mut vec = Vec::new();
        let mut current_str = &encoded_value[1..];

        //recursively call  until string exhausted.
        while current_str.len() > 0 {
            //println!("remaining: '{}'[{}]",current_str,current_str.len() );
            let (v,remaining) = decode_bencoded_value_r(current_str);
            //add element into vector for list.
            vec.push(v);
            current_str = remaining;
        }  
        //finally return the value
        return (serde_json::Value::Array(vec),"") 
    }
    else if encoded_value.chars().next().unwrap().is_digit(10) {
        return extract_string(encoded_value)
    } else if encoded_value.chars().next().unwrap() == 'i' {
        return extract_number(encoded_value)
    } else {
        panic!("Unhandled encoded value: {}", encoded_value)
    }
}



fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    let (v,remainder) = decode_bencoded_value_r(encoded_value);
    v
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
