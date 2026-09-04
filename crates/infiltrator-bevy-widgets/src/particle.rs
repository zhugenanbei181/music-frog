//! 2.5D/3D Traffic flow particle physics and spherical Great Circle navigation math.

use bevy::color::Color;
use bevy::math::{Vec2, Vec3};

/// A dynamic traffic packet particle traveling across nodes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrafficParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub life_fraction: f32, // [1.0 -> 0.0]
    pub decay_rate: f32,
    pub size_px: f32,
    pub color: Color,
}

impl TrafficParticle {
    pub fn new(start: Vec3, velocity: Vec3, lifetime_secs: f32, color: Color) -> Self {
        let decay_rate = if lifetime_secs > 0.0 {
            1.0 / lifetime_secs
        } else {
            1.0
        };
        Self {
            position: start,
            velocity,
            life_fraction: 1.0,
            decay_rate,
            size_px: 4.0,
            color,
        }
    }

    /// Step simulation forward by dt seconds. Returns true if particle is still alive.
    pub fn step(&mut self, dt: f32) -> bool {
        self.position += self.velocity * dt;
        self.life_fraction -= self.decay_rate * dt;
        self.life_fraction > 0.0
    }
}

/// Particle emitter generating traffic packet bursts along routing links.
#[derive(Clone, Debug, Default)]
pub struct ParticleEmitter {
    pub particles: Vec<TrafficParticle>,
    pub max_capacity: usize,
    pub is_active: bool,
}

impl ParticleEmitter {
    pub fn new(max_capacity: usize) -> Self {
        Self {
            particles: Vec::with_capacity(max_capacity),
            max_capacity,
            is_active: true,
        }
    }

    pub fn emit(&mut self, particle: TrafficParticle) -> bool {
        if !self.is_active || self.particles.len() >= self.max_capacity {
            return false;
        }
        self.particles.push(particle);
        true
    }

    pub fn update(&mut self, dt: f32) {
        self.particles.retain_mut(|p| p.step(dt));
    }

    pub fn count(&self) -> usize {
        self.particles.len()
    }

    pub fn clear(&mut self) {
        self.particles.clear();
    }
}

/// Spherical Great Circle Arc math between two geographic coordinates (Latitude, Longitude in degrees).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GreatCircleArc {
    pub from_lat_lon: Vec2,
    pub to_lat_lon: Vec2,
    pub radius: f32,
}

impl GreatCircleArc {
    pub fn new(from_deg: Vec2, to_deg: Vec2, radius: f32) -> Self {
        Self {
            from_lat_lon: from_deg,
            to_lat_lon: to_deg,
            radius,
        }
    }

    /// Convert (lat_deg, lon_deg) to 3D Cartesian coordinates on sphere of radius R.
    pub fn lat_lon_to_cartesian(lat_deg: f32, lon_deg: f32, radius: f32) -> Vec3 {
        let lat_rad = lat_deg.to_radians();
        let lon_rad = lon_deg.to_radians();
        let x = radius * lat_rad.cos() * lon_rad.cos();
        let y = radius * lat_rad.sin();
        let z = radius * lat_rad.cos() * lon_rad.sin();
        Vec3::new(x, y, z)
    }

    /// Sample a 3D point along the great circle arc with altitude elevation arch.
    pub fn sample_point(&self, t: f32, max_altitude_fraction: f32) -> Vec3 {
        let t = t.clamp(0.0, 1.0);
        let p0 = Self::lat_lon_to_cartesian(self.from_lat_lon.x, self.from_lat_lon.y, self.radius);
        let p1 = Self::lat_lon_to_cartesian(self.to_lat_lon.x, self.to_lat_lon.y, self.radius);

        // Spherical linear interpolation (Slerp)
        let dot = (p0.normalize().dot(p1.normalize())).clamp(-1.0, 1.0);
        let theta = dot.acos();

        let base_point = if theta.abs() < 1e-4 {
            p0
        } else {
            let sin_theta = theta.sin();
            let w0 = ((1.0 - t) * theta).sin() / sin_theta;
            let w1 = (t * theta).sin() / sin_theta;
            p0 * w0 + p1 * w1
        };

        // Add parabolic arc elevation
        let altitude = (4.0 * t * (1.0 - t)) * (self.radius * max_altitude_fraction);
        base_point.normalize() * (self.radius + altitude)
    }
}
