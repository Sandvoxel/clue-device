use std::error::Error;
use std::fmt::{Display, Formatter};
use std::{fmt, fs};
use std::env::current_dir;
use std::fs::File;
use std::io::Read;

use log::{debug, error};
use tiny_http::{Header, Request, Response};
use serde::Deserialize;
use crate::video_handler::media_manager::Command::PlayMedia;
use crate::video_handler::media_manager::VlcManager;
use crate::web_server::file_action_handler::ActionFormError::{FailedToDecodeForm, FailedToDelete, IoError};
use crate::web_server::file_action_handler::Actions::{Delete, Download, PairToCard, Play};

pub fn route_action_form(mut request: Request, media_manager: &VlcManager) -> Result<Actions, ActionFormError> {
    // Read form data
    let mut raw_form_data = String::new();
    request.as_reader().read_to_string(&mut raw_form_data).unwrap();
    debug!("Raw form data: {:?}", raw_form_data);
    let project_dir = current_dir()?;

    match serde_urlencoded::from_str::<FormData>(&raw_form_data) {
        Ok(form_data) =>{
            debug!("Parsed form data: {:?}", form_data);
            let media_dir = project_dir.join("files").join(form_data.info.clone());

            match form_data.action {
                PairToCard => {
                    request.respond(Response::from_string("paired card"))?;
                    Ok(PairToCard)
                }
                Play => {
                    media_manager.send_command(PlayMedia(media_dir)).unwrap_or_else(|error|{
                        error!("Failed to send play command to media manager: {:?}", error);
                    });
                    request.respond(Response::from_string("played video"))?;
                    Ok(Play)
                }
                Download => {
                    let mut file = File::open(media_dir)?;
                    let mut file_content = Vec::new();
                    file.read_to_end(&mut file_content)?;

                    // Create a response with the file's content
                    let response = Response::from_data(file_content)
                        .with_header("Content-Type: application/octet-stream".parse::<Header>().unwrap())
                        .with_header(format!("Content-Disposition: attachment; filename={}", form_data.info).parse::<Header>().unwrap());

                    // Send the response
                    request.respond(response)?;
                    Ok(Download)
                }
                Delete => {
                    if let Err(error) = fs::remove_file(media_dir.clone()) {
                        error!("Failed to remove file: {}", error);
                        request.respond(Response::from_string("").with_status_code(400))?;
                        return Err(FailedToDelete(media_dir.display().to_string()));
                    } else {
                        request.respond(Response::from_string("Removed File")).unwrap();
                        Ok(Delete)
                    }
                }
            }
        }
        Err(error) => {
            error!("Failed to parse string from form: {:?}", error);
            request.respond(Response::from_string("Invalid form").with_status_code(400)).unwrap_or_else(|error|{
                error!("Failed to send response to client: {:?}", error);
            });
            Err(FailedToDecodeForm)
        }
    }

    //
}

#[derive(Debug)]
pub enum ActionFormError {
    FailedToDelete(String),
    IoError(std::io::Error),
    FailedToDecodeForm
}
impl Display for ActionFormError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            FailedToDelete(file) => {write!(f, "Failed to delete file: {}", file)}
            FailedToDecodeForm => {write!(f, "Failed to decode form data")}
            IoError(error) => {write!(f, "Io operation failed: {}", error)}
        }
    }
}
impl From<std::io::Error> for ActionFormError {
    fn from(error: std::io::Error) -> Self {
        IoError(error)
    }
}
impl Error for ActionFormError {
    // Optionally, you can add more methods to provide more details about the error.
}



#[derive(Debug, Deserialize)]
struct FormData {
    info: String,
    action: Actions,
}
#[derive(Debug, Deserialize)]
pub enum Actions {
    PairToCard,
    Play,
    Download,
    Delete
}