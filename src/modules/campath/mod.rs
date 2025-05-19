//! Campath

use std::path::PathBuf;

use self::bvh::{read_bvh, Bvh};
use self::camio::CamIO;
use self::exporter::Exporter;
use super::commands::Command;
use super::Module;
use crate::handler;
use crate::hooks::engine::{self, con_print};
use crate::modules::campath::camio::read_camio;
use crate::modules::cvars::CVar;
use crate::utils::*;

mod bvh;
mod camio;
mod common;
mod exporter;

pub struct Campath;
impl Module for Campath {
    fn name(&self) -> &'static str {
        "Campath"
    }

    fn description(&self) -> &'static str {
        "Loading and exporting campath motion .cam or .bvh by HLAE and its Blender addon."
    }

    fn commands(&self) -> &'static [&'static Command] {
        static COMMANDS: &[&Command] = &[
            &BXT_CAMPATH_FORCE_LIVE,
            &BXT_CAMPATH_EXPORT_START,
            &BXT_CAMPATH_EXPORT_STOP,
        ];
        COMMANDS
    }

    fn cvars(&self) -> &'static [&'static CVar] {
        static CVARS: &[&CVar] = &[&BXT_CAMPATH_OFFSET, &BXT_CAMPATH_LOAD, &BXT_CAMPATH_ROTATE];
        CVARS
    }

    fn is_enabled(&self, marker: MainThreadMarker) -> bool {
        engine::CL_PlayDemo_f.is_set(marker)
            && engine::CL_Disconnect.is_set(marker)
            && engine::CL_ViewDemo_f.is_set(marker)
            && engine::cls_demos.is_set(marker)
            && engine::host_frametime.is_set(marker)
            && engine::r_refdef_vieworg.is_set(marker)
            && engine::r_refdef_viewangles.is_set(marker)
            && engine::R_RenderView.is_set(marker)
            && engine::scr_fov_value.is_set(marker)
            && engine::V_RenderView.is_set(marker)
    }
}

static BXT_CAMPATH_LOAD: CVar = CVar::new(
    b"bxt_campath_load\0",
    b"\0",
    "\
Loads campath .cam or .bvh file for playback during demo.
",
);

static BXT_CAMPATH_OFFSET: CVar = CVar::new(
    b"bxt_campath_offset\0",
    b"-0.25\0",
    "\
Offsets time values in second of each campath entry. Offset can be negative. Default is -0.25.
",
);

static BXT_CAMPATH_ROTATE: CVar = CVar::new(
    b"bxt_campath_rotate\0",
    b"0\0",
    "\
Rotates all camera points around origin by Z up axis. HLAE CAM format is mainly for Source. But, Source horizontal rotation is slightly different from GoldSrc.

If you want to import campath created in Blender based on map file exported from TrenchBroom or jack, you should set this value to 90. 

If you want to export campath to Blender based on map file exported from TrenchBroom or jack, you should set this value to -90.

Other tools such as Nem's Crafty are meant for Source. Its .OBJ export implicitly adds rotation. Therefore, you don't need to add rotation.
",
);

#[derive(Clone)]
enum Mdt {
    Bvh(Bvh),
    CamIO(CamIO),
}

#[derive(Clone)]
enum State {
    Idle,
    Loaded(Mdt),
    Exporting(Exporter),
}

static STATE: MainThreadRefCell<State> = MainThreadRefCell::new(State::Idle);

pub fn load(marker: MainThreadMarker) {
    if !Campath.is_enabled(marker) {
        return;
    }

    if BXT_CAMPATH_LOAD.to_string(marker).is_empty() {
        return;
    }

    reset(marker);

    let pathbuf = PathBuf::from(BXT_CAMPATH_LOAD.to_os_string(marker));

    if let Ok(file) = std::fs::read_to_string(&pathbuf) {
        match pathbuf.extension() {
            None => con_print(marker, "Error: Campath file does not have an extension.\n"),
            Some(ext) => {
                if ext == "bvh" {
                    match read_bvh(&file) {
                        Ok((_, campathinfo)) => {
                            if campathinfo.campaths.is_empty() {
                                return;
                            }
                            *STATE.borrow_mut(marker) = State::Loaded(Mdt::Bvh(campathinfo));
                        }
                        Err(_) => con_print(marker, "Error: Cannot parse .bvh.\n"),
                    }
                } else if ext == "cam" {
                    match read_camio(&file) {
                        Ok((_, campathinfo)) => {
                            if campathinfo.campaths.is_empty() {
                                return;
                            }
                            *STATE.borrow_mut(marker) = State::Loaded(Mdt::CamIO(campathinfo));
                        }
                        Err(_) => con_print(marker, "Error: Cannot parse .cam.\n"),
                    }
                } else {
                    con_print(
                        marker,
                        "Error: File extension is neither \".cam\" nor \".bvh\".\n",
                    );
                }
            }
        }
    } else {
        con_print(marker, "Error: Cannot open campath file.\n");
    }
}

// Used for both loading and exporting
static TIME: MainThreadCell<f64> = MainThreadCell::new(0.);

static BXT_CAMPATH_FORCE_LIVE: Command = Command::new(
    b"bxt_campath_force_live\0",
    handler!(
        "bxt_campath_force_live

Forces campath to execute and reset related states. Useful for playing back specifically during live gameplay.
",
        force_live as fn(_)
    ),
);

static IS_FORCED: MainThreadCell<bool> = MainThreadCell::new(false);

fn force_live(marker: MainThreadMarker) {
    if !Campath.is_enabled(marker) {
        return;
    }

    load(marker);
    IS_FORCED.set(marker, true);
}

fn rotate_round_z(point: glam::Vec3, rad: f32) -> glam::Vec3 {
    // only change x and y
    let orig_x = point.x;
    let orig_y = point.y;

    let x = orig_x * rad.cos() - orig_y * rad.sin();
    let y = orig_x * rad.sin() + orig_y * rad.cos();

    glam::vec3(x, y, point.z)
}

pub fn override_view(marker: MainThreadMarker) {
    if !Campath.is_enabled(marker) {
        return;
    }

    if matches!(*STATE.borrow(marker), State::Idle) {
        return;
    }

    if unsafe { &*engine::cls_demos.get(marker) }.demoplayback == 0 && !IS_FORCED.get(marker) {
        return;
    }

    let r_refdef_vieworg = unsafe { &mut *engine::r_refdef_vieworg.get(marker) };
    let r_refdef_viewangles = unsafe { &mut *engine::r_refdef_viewangles.get(marker) };

    let rotation_z = BXT_CAMPATH_ROTATE.as_f32(marker);

    let mut done = false;
    if let State::Loaded(ref mut mdt) = *STATE.borrow_mut(marker) {
        match mdt {
            Mdt::Bvh(mdt) => {
                match mdt.get_view(TIME.get(marker) - BXT_CAMPATH_OFFSET.as_f32(marker) as f64) {
                    Some(cam) => {
                        let rotated_vieworg = rotate_round_z(cam.vieworg, rotation_z.to_radians());
                        let mut rotated_viewangles = cam.viewangles;
                        rotated_viewangles[1] += rotation_z;

                        for i in 0..3 {
                            r_refdef_vieworg[i] = rotated_vieworg[i];
                            r_refdef_viewangles[i] = rotated_viewangles[i];
                        }
                    }
                    // If there is no campath to override, it is done.
                    None => done = true,
                }
            }
            Mdt::CamIO(mdt) => {
                match mdt.get_view(TIME.get(marker) - BXT_CAMPATH_OFFSET.as_f32(marker) as f64) {
                    Some(cam) => {
                        let rotated_vieworg =
                            rotate_round_z(cam.viewinfo.vieworg, rotation_z.to_radians());
                        let mut rotated_viewangles = cam.viewinfo.viewangles;
                        rotated_viewangles[1] += rotation_z;

                        for i in 0..3 {
                            r_refdef_vieworg[i] = rotated_vieworg[i];
                            r_refdef_viewangles[i] = rotated_viewangles[i];
                        }

                        unsafe { *engine::scr_fov_value.get(marker) = cam.fov };
                    }
                    None => done = true,
                }
            }
        }
    }

    if done {
        reset(marker);
    }
}

static BXT_CAMPATH_EXPORT_START: Command = Command::new(
    b"bxt_campath_export_start\0",
    handler!(
        "bxt_campath_export_start [filename.cam]

Starts capturing player's position and viewangle from either gameplay or demo into .cam format.

Automatically stops when demo stops.

When capturing motion from demo, in order to speed up, try `timedemo \"demoname\"`.",
        export_start as fn(_),
        export_start_with_filename as fn(_, _)
    ),
);

fn export_start(marker: MainThreadMarker) {
    export_start_with_filename(marker, "output.cam".to_string());
}

fn export_start_with_filename(marker: MainThreadMarker, filename: String) {
    if !Campath.is_enabled(marker) {
        return;
    }

    if !filename.ends_with(".cam") {
        con_print(marker, "Error: File name must end with \".cam\".\n");
        return;
    }

    // TODO: make export possible when motion load exists while there's still more.
    // Look at the comment in on_cl_disconnect()
    if !matches!(*STATE.borrow_mut(marker), State::Idle) {
        // Do not capture while loaded or exporting.
        con_print(marker, "Error: Currently loaded or exporting");
        return;
    }

    con_print(
        marker,
        &format!("Recording player motion into {}.\n", &filename),
    );

    reset(marker);
    *STATE.borrow_mut(marker) = State::Exporting(Exporter::new(filename));
}

pub fn capture_motion(marker: MainThreadMarker) {
    // Because R_RenderView is called multiple times before a frame is processed,
    // it is better to hook this with V_RenderView or something else.
    if !Campath.is_enabled(marker) {
        return;
    }

    if matches!(*STATE.borrow(marker), State::Idle) {
        return;
    }

    if let State::Exporting(ref mut exporter) = *STATE.borrow_mut(marker) {
        let r_refdef_vieworg = unsafe { &mut *engine::r_refdef_vieworg.get(marker) };
        let r_refdef_viewangles = unsafe { &mut *engine::r_refdef_viewangles.get(marker) };
        let fov = unsafe { *engine::scr_fov_value.get(marker) };

        let rotation_z = BXT_CAMPATH_ROTATE.as_f32(marker);
        let rotated_vieworg = rotate_round_z(
            glam::Vec3::from_slice(r_refdef_vieworg),
            rotation_z.to_radians(),
        );

        exporter.append_entry(
            TIME.get(marker),
            rotated_vieworg,
            [
                r_refdef_viewangles[2], // flip the order
                r_refdef_viewangles[0],
                r_refdef_viewangles[1] + rotation_z,
            ]
            .into(),
            fov,
        );
    }
}

static BXT_CAMPATH_EXPORT_STOP: Command = Command::new(
    b"bxt_campath_export_stop\0",
    handler!(
        "bxt_campath_export_stop
        
Stops capturing player motion.",
        export_stop as fn(_)
    ),
);

fn export_stop(marker: MainThreadMarker) {
    if !Campath.is_enabled(marker) {
        return;
    }

    let should_reset = if let State::Exporting(ref mut exporter) = *STATE.borrow_mut(marker) {
        exporter.write();
        con_print(marker, "Stopped capturing player motion.\n");

        // CL_PlayDemo_f has a CL_Disconnect call inside before reading the demo.
        // Should check for this otherwise state will reset.
        true
    } else {
        false
    };

    if should_reset {
        reset(marker);
    }
}

pub fn on_cl_disconnect(marker: MainThreadMarker) {
    // CL_Disconnect is called once before demo/map is loaded.
    // So it is not reliable to mark where to stop for loading campath.
    if !Campath.is_enabled(marker) {
        return;
    }

    export_stop(marker);
}

pub fn update_time(marker: MainThreadMarker) {
    if !Campath.is_enabled(marker) {
        return;
    }

    TIME.set(
        marker,
        TIME.get(marker) + unsafe { *engine::host_frametime.get(marker) },
    );
}

fn reset(marker: MainThreadMarker) {
    TIME.set(marker, 0.);
    *STATE.borrow_mut(marker) = State::Idle;
    IS_FORCED.set(marker, false);
}
