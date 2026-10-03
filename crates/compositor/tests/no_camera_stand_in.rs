//! Sans caméra, la preview compose partout avec la frame écran à la place de la webcam : le
//! décodeur remplaçant n'est plus ni avancé ni recherché (`live::open_webcam_or_stand_in`).
//!
//! Couvre les quatre chemins qui composent : le seek en pause (`present_frame`), y compris
//! au-delà de la dernière image, la lecture libre (`step`), et la recomposition à l'arrêt
//! (`recompose`). Une frame webcam nulle les ferait tous rendre `false`, preview figée.
//!
//! La CI ne le joue pas : il faut un GPU et une source vidéo, sinon il se saute. N'importe quel
//! MP4 d'au moins 3 s convient :
//!
//! ```sh
//! OPENSCREEN_LIVE_SOURCE=/chemin/source.mp4 cargo test -p openscreen-compositor --test no_camera_stand_in -- --nocapture
//! ```

#![cfg(any(windows, target_os = "macos"))]

use openscreen_compositor::compositor::Compositor;
use openscreen_compositor::config;
use openscreen_compositor::d3d::Gpu;
use openscreen_compositor::frame_geometry::live_params_from_scene;
use openscreen_compositor::live::Player;
use openscreen_compositor::scene::Scene;

fn scene_json(source: &str) -> String {
    let s = source.replace('\\', "/");
    format!(
        r##"{{
        "clips": [{{"screenPath":"{s}","webcamPath":"","sourceStartSec":0,"sourceEndSec":100000,"webcamOffsetSec":0,"hasAudio":false}}],
        "layout": {{"preset":"no-webcam","webcamSize":1.0,"webcamShape":"rounded","webcamMirror":false,"webcamPosition":null,"webcamReactiveZoom":false}},
        "effects": {{"padding":0.1,"blur":false,"shadow":0.0,"roundnessFrac":0,"motionBlur":0.0}},
        "background": {{"kind":"color","color":"#303030"}},
        "zoomRegions": [],
        "speedRegions": [],
        "cursor": {{"show":false,"size":1,"smoothing":0,"motionBlur":0,"clickBounce":0,"clipToBounds":false,"theme":"default"}},
        "cropByClip": [null],
        "output": {{"width":480,"height":270,"fps":30}}
    }}"##
    )
}

#[test]
fn every_compose_path_works_without_a_camera() {
    let Ok(source) = std::env::var("OPENSCREEN_LIVE_SOURCE") else {
        println!("SKIP: definir OPENSCREEN_LIVE_SOURCE (voir l'en-tete du fichier).");
        return;
    };
    let gpu = Gpu::create(false).expect("device");
    let mut cfg = config::all().pop().expect("au moins une config");
    cfg.zoom = false;
    cfg.layout_anim = false;
    let comp = Compositor::new_sized(&gpu, 480, 270).expect("compositor");
    let scene = Scene::from_json(&scene_json(&source)).expect("scene valide");
    comp.set_live_params(live_params_from_scene(&scene));
    comp.set_scene(Some(scene.clone()));
    comp.clear_cursor();

    unsafe {
        let mut player = Player::open(&source, "", &gpu).expect("ouvrir la source");
        assert!(!player.webcam_decoder_is_real(), "pas de caméra déclarée : décodeur remplaçant");
        player.set_programme_clock(Some(&scene), 0);

        assert!(player.present_frame(&comp, &cfg, 1.0).expect("seek"), "seek en pause");
        let start = player.screen_time_sec();

        // Lecture libre à 2× sur une seconde d'horloge, au pas de 1/60 s.
        let mut target = start;
        let mut composed = 0;
        for _ in 0..60 {
            target += 2.0 / 60.0;
            while player.step(&comp, &cfg, target).expect("step") {
                composed += 1;
            }
        }
        assert!(composed > 0, "la lecture libre n'a composé aucune frame");
        assert!(
            player.screen_time_sec() > start + 1.5,
            "la lecture libre a avancé de {:.3} s au lieu de ~2 s",
            player.screen_time_sec() - start
        );

        assert!(player.recompose(&comp, &cfg).expect("recompose"), "recomposition à l'arrêt");

        // Au-delà de la dernière image : la dernière image, composée.
        assert!(player.present_frame(&comp, &cfg, 100_000.0).expect("seek fin"), "seek au-delà de la fin");
    }
}
