use std::collections::HashMap;
use log4rs::Config;
use log4rs::config::{InitError, RawConfig};
use qoollo_log4rs_logstash::config::DeserializersExt;
use serde_json::Value;

use crate::config::setup::DeviceConfiguration;

pub fn setup_logging(device_config: &DeviceConfiguration) -> Result<(),  Box<dyn std::error::Error>> {
    let device_id = device_config.device_uuid.clone();
    let binding = include_str!("../../config/log4rs.yaml").replace("{device_id}", device_id.as_str());
    let config_str = binding.as_str();
    let config: RawConfig = serde_yaml::from_str(config_str).unwrap();

    let mut data:  HashMap<String, Value>  = HashMap::new();

    data.insert("device-id".to_string(), Value::String(device_config.device_uuid.clone()));

    let (appenders, errors) = config.appenders_lossy(&log4rs::config::Deserializers::default().with_logstash_extra(data));
    if !errors.is_empty() {
        panic!();
    }
    let config = Config::builder()
        .appenders(appenders)
        .loggers(config.loggers())
        .build(config.root())?;


    log4rs::init_config(config).unwrap();

    Ok(())
}