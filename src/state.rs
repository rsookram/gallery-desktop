use std::{ffi::OsString, path::PathBuf};

use crate::{selector, viewer};

pub struct State {
    pub width: i32,
    pub height: i32,
    pub screen: Screen,
}

pub enum Screen {
    Selector(selector::Screen),
    Viewer(viewer::Screen),
}

impl State {
    pub fn new(args: Vec<OsString>) -> Self {
        let screen = match args.first().map(|arg| arg.as_encoded_bytes()) {
            Some(b"--select") | Some(b"-s") => {
                let mut args = args.into_iter();
                args.next();

                let paths = args.map(PathBuf::from).collect();
                Screen::Selector(selector::Screen::new(paths))
            }
            _ => {
                let paths = args.into_iter().map(PathBuf::from).collect();
                Screen::Viewer(viewer::Screen::new(paths))
            }
        };

        Self {
            width: 0,
            height: 0,
            screen,
        }
    }

    pub fn move_to_viewer(&mut self) {
        let Screen::Selector(screen) = &self.screen else {
            return;
        };

        self.screen = Screen::Viewer(viewer::Screen::new(
            screen
                .ofcs
                .iter()
                .filter(|ofc| ofc.selected)
                .map(|ofc| ofc.path.clone())
                .collect(),
        ));
    }
}
