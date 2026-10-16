//! docs/features/bevy-roles.md RD-9 ("Role facts": `BevyAnalysis` gains
//! fields; `CallSites` and the eval's `bevy-detector` output stay byte for
//! byte): the facts 10a-1 adds (derives, `impl Component for`,
//! `add_message`, in code and in macro tokens, `cfg_attr` derives) leave
//! the shipped fields of `bevy::detect` exactly as they were. The eval's
//! half is `specengine-eval`'s `bevy_cli.rs` over `fixtures/bevy-mini`,
//! byte for byte (TE-6).
//!
//! Every input is generated here, through the shipped API only
//! (`bevy::detect_file`, the shipped fields): the new facts are read by
//! the index tests
//! (`specengine-cli`'s `bevy_roles.rs`) through the stored rows.

use specengine_code::RustParser;
use specengine_code::bevy::{self, BevyAnalysis};

fn detect(source: &str) -> BevyAnalysis {
    let mut parser = RustParser::new().expect("grammar loads");
    bevy::detect_file(&mut parser, source).expect("parse is never cancelled")
}

/// Every shipped fact kind: registrations of each form (a pipe, a
/// factory, a closure), an observer, plugin uses, `impl Plugin for`, an
/// `&mut App` fn, two uncertain constructs (a macro in a system position,
/// a metavariable).
const SHIPPED: &str = r#"use bevy::prelude::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (tick, sense.pipe(react), spawn(3), |q: Query<&Crop>| {}))
            .add_observer(on_hit)
            .add_plugins((DefaultPlugins, HudPlugin::default()));
        app.add_systems(Update, my_systems!());
    }
}

fn hud(app: &mut App) {}

macro_rules! register {
    ($app:expr, $sys:ident) => {
        $app.add_systems(PostUpdate, $sys);
    };
}
"#;

/// Only what 10a-1 adds, after the shipped source (its lines unchanged):
/// derives of every form, `impl Component for`, `add_message` in code (in
/// a fn of two parameters: no plugin) and in a transcriber.
const NEW_FACTS: &str = r#"
#[derive(Component, Resource, Message, Event, EntityEvent, Reflect)]
pub struct Every;

#[derive(bevy::prelude::Component, Debug)]
#[derive(Clone)]
#[cfg_attr(feature = "net", derive(Resource))]
pub struct Other;

pub struct Tractor;

impl Component for Tractor {
    type Mutability = Mutable;
}

fn messages(app: &mut App, count: u8) {
    app.add_message::<Harvest>()
        .add_message::<news::Quake<u8>>();
}

macro_rules! news {
    ($app:expr) => {
        $app.add_message::<Rain>();
    };
}
"#;

/// RD-9. M: a shipped registration changed; the new facts counted as call
/// sites or uncertain constructs. With the new facts appended, the shipped
/// fields equal the shipped source's alone.
#[test]
fn new_facts_leave_the_shipped_fields_unchanged() {
    let before = detect(SHIPPED);
    assert!(!before.registrations.is_empty(), "{before:?}");
    assert!(!before.plugins.is_empty(), "{before:?}");
    assert!(!before.plugin_uses.is_empty(), "{before:?}");
    assert_eq!(before.uncertain.len(), 2, "{:?}", before.uncertain);
    let after = detect(&format!("{SHIPPED}{NEW_FACTS}"));
    assert_eq!(after.registrations, before.registrations);
    assert_eq!(after.plugins, before.plugins);
    assert_eq!(after.plugin_uses, before.plugin_uses);
    assert_eq!(after.uncertain, before.uncertain);
    assert_eq!(after.call_sites, before.call_sites);
    assert_eq!(after.has_error, before.has_error);
}

/// RD-9 (reading: an unreadable `add_message` is a role row's, never a
/// shipped `Uncertain`, whose `call` names the three shipped methods). A
/// metavariable type in a transcriber's `add_message` leaves the shipped
/// fields empty.
#[test]
fn an_unreadable_add_message_stays_out_of_the_shipped_uncertain() {
    let source = r#"macro_rules! register {
    ($app:expr, $t:ty) => {
        $app.add_message::<$t>();
    };
}
"#;
    let analysis = detect(source);
    assert!(analysis.registrations.is_empty(), "{analysis:?}");
    assert!(analysis.plugins.is_empty(), "{analysis:?}");
    assert!(analysis.plugin_uses.is_empty(), "{analysis:?}");
    assert!(analysis.uncertain.is_empty(), "{analysis:?}");
    assert_eq!(analysis.call_sites, bevy::CallSites::default());
}
