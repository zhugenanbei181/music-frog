//! The globe geometry: the polar projection, the great-circle arc and the
//! flight light of a cross-border connection (EFFECTS-013 #46, #47).
//!
//! The globe is a sphere, not a flat map: a coordinate is a latitude and a
//! longitude, and the drawing is a point in three dimensions. The
//! conversion lives here so the two surfaces place the same node on the same
//! spot of the same sphere, and so the flight light of a cross-border
//! connection follows the same arc everywhere.

/// One point of the globe, in three dimensions.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpherePoint {
    /// The horizontal coordinate.
    pub x: f64,
    /// The depth coordinate.
    pub y: f64,
    /// The vertical coordinate.
    pub z: f64,
}

impl SpherePoint {
    /// One point.
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// The distance from the centre of the globe.
    pub fn radius(self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

/// The point of the globe a latitude and a longitude name.
///
/// The globe's axis is the z axis: the north pole is `(0, 0, r)`, the
/// equator on the prime meridian is `(r, 0, 0)` and the equator a quarter
/// turn east is `(0, r, 0)`.
pub fn lat_lon_to_sphere(latitude_deg: f64, longitude_deg: f64, radius: f64) -> SpherePoint {
    // A latitude is an angle at the pole: a value beyond ±90° describes no
    // place on the globe, so it is clamped to the pole it names instead of
    // wrapping around to the far side.
    let latitude = latitude_deg.clamp(-90.0, 90.0).to_radians();
    let longitude = longitude_deg.to_radians();
    let (sin_lat, cos_lat) = latitude.sin_cos();
    let (sin_lon, cos_lon) = longitude.sin_cos();
    SpherePoint::new(
        radius * cos_lat * cos_lon,
        radius * cos_lat * sin_lon,
        radius * sin_lat,
    )
}

/// The points of the great-circle arc between two points of the globe.
///
/// The arc is sampled at the spherical interpolation of the two points, so
/// every sample lies on the sphere and on the arc.
pub fn great_circle_arc(start: SpherePoint, end: SpherePoint, segments: usize) -> Vec<SpherePoint> {
    if segments == 0 {
        return Vec::new();
    }
    let start_radius = start.radius();
    let end_radius = end.radius();
    let start_angle = start_radius.max(f64::MIN_POSITIVE);
    let end_angle = end_radius.max(f64::MIN_POSITIVE);
    let dot = (start.x * end.x + start.y * end.y + start.z * end.z) / (start_angle * end_angle);
    let omega = dot.clamp(-1.0, 1.0).acos();
    if omega.abs() < f64::EPSILON {
        return vec![start; segments + 1];
    }
    let sin_omega = omega.sin();
    let mut arc = Vec::with_capacity(segments + 1);
    for step in 0..=segments {
        let t = step as f64 / segments as f64;
        let weight_start = ((1.0 - t) * omega).sin() / sin_omega;
        let weight_end = (t * omega).sin() / sin_omega;
        arc.push(SpherePoint::new(
            weight_start * start.x + weight_end * end.x,
            weight_start * start.y + weight_end * end.y,
            weight_start * start.z + weight_end * end.z,
        ));
    }
    arc
}

/// The flight path of one cross-border connection (EFFECTS-013 #47).
///
/// The path is the quadratic Bézier curve that lifts the connection off the
/// surface of the globe between its two ends, and the flight light is the
/// point on that curve that travels from one end to the other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightPath {
    /// Where the connection starts.
    pub start: SpherePoint,
    /// The control point that lifts the flight off the surface.
    pub control: SpherePoint,
    /// Where the connection ends.
    pub end: SpherePoint,
}

impl FlightPath {
    /// The path between two points of the globe, lifted by `arc_height`.
    ///
    /// The control point is placed so that the *middle* of the curve sits
    /// `arc_height` above the surface: the flight leaves the globe and lands
    /// on it again instead of cutting through it.
    pub fn between(start: SpherePoint, end: SpherePoint, arc_height: f64) -> Self {
        let radius = (start.radius() + end.radius()) / 2.0;
        let middle = SpherePoint::new(
            (start.x + end.x) / 2.0,
            (start.y + end.y) / 2.0,
            (start.z + end.z) / 2.0,
        );
        let chord = middle.radius();
        if chord <= f64::MIN_POSITIVE {
            return Self {
                start,
                control: SpherePoint::new(0.0, 0.0, radius + arc_height),
                end,
            };
        }
        // The middle of a quadratic curve is (start + 2 * control + end) / 4,
        // so the control distance follows from the height the arc must reach.
        let control_distance = 2.0 * (radius + arc_height) - chord;
        let control = SpherePoint::new(
            middle.x / chord * control_distance,
            middle.y / chord * control_distance,
            middle.z / chord * control_distance,
        );
        Self {
            start,
            control,
            end,
        }
    }

    /// The point of the flight at the phase `t`, in 0.0..=1.0.
    pub fn point(&self, t: f32) -> SpherePoint {
        let t = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        // The sphere points are f64, so the curve is evaluated in f64 too.
        let phase = t as f64;
        let inverse = 1.0 - phase;
        let a = inverse * inverse;
        let b = 2.0 * inverse * phase;
        let c = phase * phase;
        SpherePoint::new(
            a * self.start.x + b * self.control.x + c * self.end.x,
            a * self.start.y + b * self.control.y + c * self.end.y,
            a * self.start.z + b * self.control.z + c * self.end.z,
        )
    }

    /// The phase of the flight light at `elapsed_ms`.
    ///
    /// The phase repeats every `period_ms`, so the light travels from one end
    /// to the other and starts again.
    pub fn light_phase(elapsed_ms: f32, period_ms: f32) -> f32 {
        if !elapsed_ms.is_finite() || !period_ms.is_finite() || period_ms <= 0.0 {
            return 0.0;
        }
        (elapsed_ms.rem_euclid(period_ms) / period_ms).clamp(0.0, 1.0)
    }
}

/// The number of degrees of a full turn.
pub const FULL_TURN_DEGREES: f64 = 360.0;

/// The half turn, in degrees.
pub const HALF_TURN_DEGREES: f64 = 180.0;

/// The quarter turn, in degrees.
pub const QUARTER_TURN_DEGREES: f64 = 90.0;

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_1_SQRT_2;

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, measured {actual}"
        );
    }

    #[test]
    fn the_polar_conversion_is_exact() {
        let point = lat_lon_to_sphere(0.0, 0.0, 1.0);
        close(point.x, 1.0);
        close(point.y, 0.0);
        close(point.z, 0.0);

        let point = lat_lon_to_sphere(0.0, QUARTER_TURN_DEGREES, 1.0);
        close(point.x, 0.0);
        close(point.y, 1.0);
        close(point.z, 0.0);

        let point = lat_lon_to_sphere(HALF_TURN_DEGREES, 0.0, 1.0);
        close(point.x, 0.0);
        close(point.y, 0.0);
        close(point.z, 1.0);

        let point = lat_lon_to_sphere(-HALF_TURN_DEGREES, 0.0, 2.0);
        close(point.x, 0.0);
        close(point.y, 0.0);
        close(point.z, -2.0);

        let point = lat_lon_to_sphere(45.0, 0.0, 1.0);
        close(point.x, FRAC_1_SQRT_2);
        close(point.y, 0.0);
        close(point.z, FRAC_1_SQRT_2);
    }

    #[test]
    fn every_point_of_the_globe_keeps_its_radius() {
        for latitude in [-90.0, -45.0, 0.0, 45.0, 90.0] {
            for longitude in [-180.0, -90.0, 0.0, 90.0, 180.0] {
                let point = lat_lon_to_sphere(latitude, longitude, 3.0);
                close(point.radius(), 3.0);
            }
        }
    }

    #[test]
    fn the_great_circle_arc_starts_and_ends_on_the_globe() {
        let start = lat_lon_to_sphere(0.0, 0.0, 1.0);
        let end = lat_lon_to_sphere(0.0, 90.0, 1.0);
        let arc = great_circle_arc(start, end, 8);
        assert_eq!(arc.len(), 9);
        close(arc[0].x, start.x);
        close(arc[8].y, end.y);
        for point in &arc {
            close(point.radius(), 1.0);
        }
    }

    #[test]
    fn the_flight_light_travels_from_one_end_to_the_other() {
        let start = lat_lon_to_sphere(0.0, 0.0, 1.0);
        let end = lat_lon_to_sphere(0.0, 120.0, 1.0);
        let path = FlightPath::between(start, end, 0.4);
        let first = path.point(0.0);
        close(first.x, start.x);
        let last = path.point(1.0);
        close(last.z, end.z);
        // The flight leaves the surface in the middle.
        let middle = path.point(0.5);
        assert!(middle.radius() > 1.0);
    }

    #[test]
    fn the_light_phase_repeats() {
        assert_eq!(FlightPath::light_phase(0.0, 1000.0), 0.0);
        assert_eq!(FlightPath::light_phase(500.0, 1000.0), 0.5);
        assert_eq!(FlightPath::light_phase(1250.0, 1000.0), 0.25);
        assert_eq!(FlightPath::light_phase(100.0, 0.0), 0.0);
        assert_eq!(FlightPath::light_phase(f32::NAN, 1000.0), 0.0);
    }
}
