use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use sha1::Digest;
use sha1::Sha1;

use crate::logger::log;

pub fn generate_device_id() -> String {
    let random_data = rand::random::<u32>() as u32;
    // Time since the epoch
    let time_since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let random_data: String = format!("{}-{}", random_data, time_since_epoch);
    let mut hasher = Sha1::new();
    hasher.update(random_data.as_bytes());
    let result = hasher.finalize();
    let device_id = format!("{:X}", result);
    log("Сгенерирован deviceId: ".to_string() + device_id.as_str());
    return device_id;
}
