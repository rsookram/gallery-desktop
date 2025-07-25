use std::path::PathBuf;

use crate::file::FileContainer;

pub struct State {
    pub width: i32,
    pub height: i32,
    pub show_progress: bool,
    pub paths: Paths,
    pub current_file: CurrentFile,
}

pub struct Paths {
    pub data: Vec<PathBuf>,
    /// The index into `data` of the file to display
    pub index: usize,
}

pub struct CurrentFile {
    pub file: FileContainer,
    /// The index of the image within the current file to display
    pub index: usize,
}

impl State {
    pub fn new(paths: Vec<PathBuf>) -> Self {
        Self {
            width: 0,
            height: 0,
            current_file: CurrentFile {
                file: FileContainer::open(&paths[0]),
                index: 0,
            },
            paths: Paths {
                data: paths,
                index: 0,
            },
            show_progress: false,
        }
    }

    pub fn current_image_bytes(&mut self) -> Vec<u8> {
        self.current_file.file.read_at(self.current_file.index)
    }

    pub fn next_image(&mut self) {
        if self.current_file.index == self.current_file.file.len() - 1 {
            if self.paths.index == self.paths.data.len() - 1 {
                return;
            }

            self.paths.index += 1;
            self.current_file.file = FileContainer::open(&self.paths.data[self.paths.index]);
            self.current_file.index = 0;
        } else {
            self.current_file.index += 1;
        }
    }

    pub fn previous_image(&mut self) {
        if self.current_file.index == 0 {
            if self.paths.index == 0 {
                return;
            }

            self.paths.index -= 1;
            self.current_file.file = FileContainer::open(&self.paths.data[self.paths.index]);
            self.current_file.index = self.current_file.file.len() - 1;
        } else {
            self.current_file.index -= 1;
        }
    }

    pub fn next_file(&mut self) {
        if self.paths.index == 0 {
            return;
        }

        self.paths.index -= 1;
        self.current_file.file = FileContainer::open(&self.paths.data[self.paths.index]);
        self.current_file.index = 0;
    }

    pub fn previous_file(&mut self) {
        if self.paths.index == self.paths.data.len() - 1 {
            return;
        }

        self.paths.index += 1;
        self.current_file.file = FileContainer::open(&self.paths.data[self.paths.index]);
        self.current_file.index = 0;
    }
}
