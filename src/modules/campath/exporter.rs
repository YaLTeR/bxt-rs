use std::fs::OpenOptions;
use std::io::{BufWriter, Write};

use super::camio::{CamIO, ViewInfoCamIO};
use super::common::ViewInfo;

#[derive(Clone)]
pub struct Exporter {
    filename: String,
    data: CamIO,
}

impl Exporter {
    pub fn new(filename: String) -> Self {
        Self {
            filename,
            data: CamIO {
                campaths: Vec::new(),
            },
        }
    }

    fn header(&self) -> String {
        "\
advancedfx Cam
version 2
channels time xPosition yPosition zPositon xRotation yRotation zRotation fov
DATA
"
        .to_string()
    }

    pub fn append_entry(
        &mut self,
        time: f64,
        vieworg: glam::Vec3,
        viewangles: glam::Vec3,
        fov: f32,
    ) -> &Self {
        self.data.campaths.push(ViewInfoCamIO {
            viewinfo: ViewInfo {
                vieworg,
                viewangles,
            },
            time,
            fov,
        });
        self
    }

    fn entry_to_string(&self, idx: usize) -> String {
        let curr = self.data.campaths[idx];
        format!(
            "{} {} {} {} {} {} {} {}\n",
            curr.time,
            curr.viewinfo.vieworg[0],
            curr.viewinfo.vieworg[1],
            curr.viewinfo.vieworg[2],
            curr.viewinfo.viewangles[0],
            curr.viewinfo.viewangles[1],
            curr.viewinfo.viewangles[2],
            curr.fov
        )
    }

    pub fn write(&self) {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(self.filename.clone())
            .expect("Error: Cannot create .cam motion file.");
        let mut file = BufWriter::new(file);

        file.write_all(self.header().as_bytes())
            .expect("Error: Cannot write motion into .cam file.");
        for idx in 0..self.data.campaths.len() {
            file.write_all(self.entry_to_string(idx).as_bytes())
                .expect("Error: Cannot write motion into .cam file.");
        }
    }
}
