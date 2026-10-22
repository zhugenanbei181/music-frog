//! JNI entry points for the EFFECTS-047 host facts.
//!
//! Surface, window-configuration and pointer facts arrive here from the native
//! Activity. Each entry point only decodes the wire payload and hands it to the
//! typed owner in [`crate::native_host`]; the arbitration and lifecycle truth
//! stays there.

use crate::FfiErrorCode;
use crate::native_host::configuration::ConfigurationChange;
use crate::native_host::configuration::ConfigurationFact;
use crate::native_host::configuration::HostOrientation;
use crate::native_host::gesture::GestureOutcome;
use crate::native_host::gesture::GestureSample;
use crate::native_host::gesture::TouchPhase;
use crate::native_host::push_configuration_fact;
use crate::native_host::push_gesture_sample;
use crate::native_host::push_surface_fact;
use crate::native_host::surface::HostSurfaceState;
use crate::native_host::surface::SurfaceFact;
use crate::native_host::surface::SurfaceUpdate;
use jni::errors::LogErrorAndDefault;
use jni::objects::JObject;
use jni::sys::{jfloat, jint, jlong};
use jni::{EnvUnowned, errors};

/// EFFECTS-047 (#190): push one surface fact from the native Activity.
///
/// The Activity generation fences a retired instance, so a late callback from
/// a destroyed Activity reports the typed stale state instead of tearing down
/// the live surface.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_musicfrog_infiltrator_RustBridge_nativeOnSurface<'local>(
    mut env: EnvUnowned<'local>,
    _object: JObject<'local>,
    generation: jlong,
    state: jint,
    width_px: jint,
    height_px: jint,
) -> jint {
    let mut status = FfiErrorCode::InvalidState as jint;
    env.with_env(|_env| -> errors::Result<()> {
        let Some(state) = HostSurfaceState::from_code(state, width_px, height_px) else {
            status = FfiErrorCode::InvalidInput as jint;
            return Ok(());
        };
        if generation < 0 {
            status = FfiErrorCode::InvalidInput as jint;
            return Ok(());
        }
        let fact = SurfaceFact::new(generation as u64, state);
        status = match push_surface_fact(fact) {
            SurfaceUpdate::Applied | SurfaceUpdate::Recreated => FfiErrorCode::Ok as jint,
            SurfaceUpdate::Stale => FfiErrorCode::InvalidState as jint,
        };
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
    status
}

/// EFFECTS-047 (#192): push one window configuration fact.
///
/// The host reports the real orientation, pixel size, density and font scale
/// so the shared shell can relayout after a rotation.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_musicfrog_infiltrator_RustBridge_nativeOnConfiguration<'local>(
    mut env: EnvUnowned<'local>,
    _object: JObject<'local>,
    generation: jlong,
    orientation: jint,
    width_px: jint,
    height_px: jint,
    density: jfloat,
    font_scale: jfloat,
) -> jint {
    let mut status = FfiErrorCode::InvalidState as jint;
    env.with_env(|_env| -> errors::Result<()> {
        let Some(orientation) = HostOrientation::from_code(orientation) else {
            status = FfiErrorCode::InvalidInput as jint;
            return Ok(());
        };
        if generation < 0 || width_px < 0 || height_px < 0 {
            status = FfiErrorCode::InvalidInput as jint;
            return Ok(());
        }
        let fact = ConfigurationFact::new(
            generation as u64,
            orientation,
            width_px as u32,
            height_px as u32,
            density,
            font_scale,
        );
        status = match push_configuration_fact(fact) {
            ConfigurationChange::Stale => FfiErrorCode::InvalidState as jint,
            ConfigurationChange::Unchanged
            | ConfigurationChange::OrientationChanged { .. }
            | ConfigurationChange::SizeChanged { .. }
            | ConfigurationChange::DensityChanged { .. }
            | ConfigurationChange::FontScaleChanged { .. }
            | ConfigurationChange::Recreated => FfiErrorCode::Ok as jint,
        };
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
    status
}

/// EFFECTS-047 (#191): push one normalized pointer sample and read the typed
/// arbitration verdict. The verdict code is the answer, not an error code.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_musicfrog_infiltrator_RustBridge_nativeOnGesture<'local>(
    mut env: EnvUnowned<'local>,
    _object: JObject<'local>,
    pointer_id: jlong,
    phase: jint,
    x: jfloat,
    y: jfloat,
    time_millis: jlong,
) -> jint {
    let mut verdict = GestureOutcome::Undecided.as_code() as jint;
    env.with_env(|_env| -> errors::Result<()> {
        let Some(phase) = TouchPhase::from_code(phase) else {
            return Ok(());
        };
        if time_millis < 0 {
            return Ok(());
        }
        let sample = GestureSample::new(pointer_id, phase, x, y, time_millis as u64);
        verdict = push_gesture_sample(sample).as_code() as jint;
        Ok(())
    })
    .resolve::<LogErrorAndDefault>();
    verdict
}
