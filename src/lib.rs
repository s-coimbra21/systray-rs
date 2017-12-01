// Systray Lib

#[macro_use]
extern crate log;
#[cfg(target_os = "windows")]
extern crate winapi;
#[cfg(target_os = "windows")]
extern crate kernel32;
#[cfg(target_os = "windows")]
extern crate user32;
#[cfg(target_os = "windows")]
extern crate libc;
#[cfg(target_os = "linux")]
extern crate gtk;
#[cfg(target_os = "linux")]
extern crate glib;
#[cfg(target_os = "linux")]
extern crate libappindicator;

pub mod api;

use std::thread;
use std::sync::RwLock;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender, Receiver};

#[derive(Clone, Debug)]
pub enum SystrayError {
    OsError(String),
    NotImplementedError,
    UnknownError,
}

pub enum SystrayEvent {
    MenuEvent(u32),
    Quit,
}

impl std::fmt::Display for SystrayError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            &SystrayError::OsError(ref err_str) => write!(f, "OsError: {}", err_str),
            &SystrayError::NotImplementedError => write!(f, "Functionality is not implemented yet"),
            &SystrayError::UnknownError => write!(f, "Unknown error occurrred"),
        }
    }
}

pub struct Application {
    window: api::api::Window,
    menu_idx: u32,
    callback: RwLock<HashMap<u32, Callback>>,
    tx: Sender<SystrayEvent>,
    // Each platform-specific window module will set up its own thread for
    // dealing with the OS main loop. Use this channel for receiving events from
    // that thread.
    rx: Receiver<SystrayEvent>,
}

type Callback = Box<RwLock<(Fn(Sender<SystrayEvent>) -> () + Send + Sync + 'static)>>;

fn make_callback<F>(f: F) -> Callback
    where F: std::ops::Fn(Sender<SystrayEvent>) -> () + Send + Sync + 'static
{
    Box::new(RwLock::new(f)) as Callback
}

impl Application {
    pub fn new() -> Result<Application, SystrayError> {
        let (event_tx, event_rx) = channel();
        match api::api::Window::new(event_tx.clone()) {
            Ok(w) => {
                Ok(Application {
                       window: w,
                       menu_idx: 0,
                       callback: RwLock::new(HashMap::new()),
                       tx: event_tx,
                       rx: event_rx,
                   })
            }
            Err(e) => Err(e),
        }
    }

    pub fn add_menu_item<F>(&mut self, item_name: &String, f: F) -> Result<u32, SystrayError>
        where F: std::ops::Fn(Sender<SystrayEvent>) -> () + Send + Sync + 'static
    {
        let idx = self.menu_idx;
        if let Err(e) = self.window.add_menu_entry(idx, item_name) {
            return Err(e);
        }
        self.callback
            .get_mut()
            .unwrap()
            .insert(idx, make_callback(f));
        self.menu_idx += 1;
        Ok(idx)
    }

    pub fn add_menu_separator(&mut self) -> Result<u32, SystrayError> {
        let idx = self.menu_idx;
        if let Err(e) = self.window.add_menu_separator(idx) {
            return Err(e);
        }
        self.menu_idx += 1;
        Ok(idx)
    }

    pub fn set_icon_from_file(&self, file: &String) -> Result<(), SystrayError> {
        self.window.set_icon_from_file(file)
    }

    pub fn set_icon_from_resource(&self, resource: &String) -> Result<(), SystrayError> {
        self.window.set_icon_from_resource(resource)
    }

    #[cfg(not(target_os = "linux"))]
    pub fn set_icon_from_buffer(&self,
                                buffer: &[u8],
                                height: u32,
                                width: u32)
                                -> Result<(), SystrayError> {
        self.window.set_icon_from_buffer(buffer, width, height)
    }

    pub fn shutdown(&self) -> Result<(), SystrayError> {
        self.window.shutdown()
    }

    pub fn set_tooltip(&self, tooltip: &String) -> Result<(), SystrayError> {
        self.window.set_tooltip(tooltip)
    }

    pub fn quit(&mut self) {
        self.window.quit()
    }

    pub fn wait_for_message(mut self) -> Sender<SystrayEvent> {
        let sender = self.tx.clone();
        thread::spawn(move || {
            loop {
                match self.rx.recv() { 
                    Ok(m) => {
                        match m {
                            SystrayEvent::MenuEvent(m) => {
                                if self.callback.read().unwrap().contains_key(&m) {
                                    let cb_map = self.callback.read().unwrap();
                                    let f = cb_map.get(&m).unwrap().read().unwrap();
                                    f(self.tx.clone());
                                    // cb_map.insert(msg.menu_index, f);
                                }
                            }
                            _ => {
                                break;
                            }
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
            self.quit();
        });
        sender
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        self.shutdown().ok();
    }
}
