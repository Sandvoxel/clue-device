use log4rs::Config;
use log4rs::config::{InitError, RawConfig};

use crate::config::setup::DeviceConfiguration;

pub fn setup_logging(device_config: &DeviceConfiguration) -> Result<(),  Box<dyn std::error::Error>> {
    let device_id = device_config.device_uuid.clone();
    let binding = include_str!("../../config/log4rs.yaml").replace("{device_id}", device_id.as_str());
    let config_str = binding.as_str();
    let config: RawConfig = serde_yaml::from_str(config_str).unwrap();

    let (appenders, errors) = config.appenders_lossy(log4rs::config::Deserializers::default().with_logstash());
    if !errors.is_empty() {
        return panic!();
    }
    let config = Config::builder()
        .appenders(appenders)
        .loggers(config.loggers())
        .build(config.root())?;


    log4rs::init_config(config).unwrap();

    Ok(())
}