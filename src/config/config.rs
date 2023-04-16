use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
pub struct DeviceConfiguration {
    pub device_uuid: String
}


impl DeviceConfiguration {
    pub fn new() -> DeviceConfiguration {
        DeviceConfiguration{
            device_uuid: Uuid::new_v4().to_string()
        }
    }


    pub fn load(path: PathBuf) -> DeviceConfiguration{
        let device_config: DeviceConfiguration;
        if !path.is_file() {
            // If the YAML file doesn't exist, create it and save the struct as YAML
            device_config = DeviceConfiguration::new();

            let serialized_yaml = serde_yaml::to_string(&device_config).unwrap();
            let mut file = File::create(path.clone()).expect("Unable to create file");
            file.write_all(serialized_yaml.as_bytes())
                .expect("Unable to write data to file");
            println!("Config file created: {}", path.display());
        } else {
            // If the YAML file exists, read and parse it into the struct
            let mut file = OpenOptions::new()
                .read(true)
                .open(path)
                .expect("Unable to open config");
            let mut contents = String::new();
            file.read_to_string(&mut contents).expect("Unable to read file");
            device_config = serde_yaml::from_str(&contents).expect("Failed to parse config file");
            println!("Config file read: {:?}", device_config);
        }

        device_config
    }
}