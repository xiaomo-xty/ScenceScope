#![cfg_attr(not(debug_assertions), deny(warnings))]

//! `SceneScope` desktop executable.

use scenescope_desktop::run;

fn main() -> anyhow::Result<()> {
    run()
}
