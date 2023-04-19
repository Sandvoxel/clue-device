





use log4rs::config::RawConfig;

use crate::config::setup::DeviceConfiguration;

pub fn setup_logging(device_config: &DeviceConfiguration) -> Result<(),  Box<dyn std::error::Error>> {
    let device_id = device_config.device_uuid.clone();
    let binding = include_str!("../../config/log4rs.yaml").replace("{device_id}", device_id.as_str());
    let config_str = binding.as_str();
    let config: RawConfig = serde_yaml::from_str(config_str).unwrap();

    log4rs::init_raw_config(config).unwrap();

    Ok(())
}