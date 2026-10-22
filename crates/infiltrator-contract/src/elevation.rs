//! The elevation ladder and the two-layer shadow specification (EFFECTS-002 #9).
//!
//! One elevation step is two shadows: a soft ambient aura that lifts the
//! surface off its backdrop, and a tight key shadow that gives it a contact
//! edge. The flat [`ElevationToken`] is the number set both surfaces already
//! mirror; [`ShadowSpec`] is the explicit layer structure a renderer needs.
//!
//! The level is the typed identity of a step: [`ElevationLevel`] names it,
//! the token carries it and the shadow specification repeats it, so a surface
//! that asks for "the floating elevation" always receives the same two
//! layers.

/// The elevation levels the product paints (EFFECTS-002 #9).
///
/// The level is the vocabulary a surface programs against: it asks for the
/// level and receives the shadow that level resolves to, so two surfaces
/// cannot lift the same card by different amounts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ElevationLevel {
    /// A card resting on a page.
    Low,
    /// A floating layer: a dropdown, a menu, a popover.
    Medium,
    /// A modal or a command palette.
    High,
}

impl ElevationLevel {
    /// Every elevation level, from the resting one to the highest.
    pub const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];

    /// The position of the level in the ladder.
    pub const fn index(self) -> usize {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
        }
    }

    /// The stable name of the level.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    /// The flat token this level paints.
    pub const fn token(self) -> ElevationToken {
        match self {
            Self::Low => LOW,
            Self::Medium => MEDIUM,
            Self::High => HIGH,
        }
    }
}

/// The flat description of one elevation step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElevationToken {
    /// The elevation level this token describes.
    pub level: ElevationLevel,
    /// The vertical offset of the shadow in logical pixels.
    pub offset_y: f32,
    /// The blur radius of the key shadow in logical pixels.
    pub blur_radius: f32,
    /// The spread of the shadow in logical pixels.
    pub spread: f32,
    /// The alpha of the ambient layer.
    pub ambient_alpha: f32,
    /// The alpha of the key layer.
    pub key_alpha: f32,
}

/// One layer of a shadow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowLayer {
    /// The horizontal offset of the layer in logical pixels.
    pub offset_x: f32,
    /// The vertical offset of the layer in logical pixels.
    pub offset_y: f32,
    /// The blur radius of the layer in logical pixels.
    pub blur_radius: f32,
    /// The spread of the layer in logical pixels.
    pub spread: f32,
    /// The alpha of the layer.
    pub alpha: f32,
}

/// The complete shadow of one elevation step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowSpec {
    /// The elevation level this shadow belongs to.
    pub level: ElevationLevel,
    /// The soft ambient aura around the surface.
    pub ambient: ShadowLayer,
    /// The tight key shadow that gives the surface its contact edge.
    pub key: ShadowLayer,
}

impl ShadowSpec {
    /// The two layers of the shadow, in paint order: the ambient aura first,
    /// the key shadow on top of it.
    pub const fn layers(&self) -> [ShadowLayer; 2] {
        [self.ambient, self.key]
    }
}

impl ElevationToken {
    /// The two-layer shadow this elevation paints.
    ///
    /// The ambient layer shares the key layer's geometry but reaches twice as
    /// far with its blur, which is what makes it read as an aura instead of a
    /// second shadow.
    pub const fn shadow(self) -> ShadowSpec {
        ShadowSpec {
            level: self.level,
            ambient: ShadowLayer {
                offset_x: 0.0,
                offset_y: self.offset_y,
                blur_radius: self.blur_radius * 2.0,
                spread: self.spread,
                alpha: self.ambient_alpha,
            },
            key: ShadowLayer {
                offset_x: 0.0,
                offset_y: self.offset_y,
                blur_radius: self.blur_radius,
                spread: self.spread,
                alpha: self.key_alpha,
            },
        }
    }
}

/// The resting elevation: a card on a page.
pub const LOW: ElevationToken = ElevationToken {
    level: ElevationLevel::Low,
    offset_y: 2.0,
    blur_radius: 4.0,
    spread: 0.0,
    ambient_alpha: 0.04,
    key_alpha: 0.08,
};
/// The floating elevation: a dropdown or a menu.
pub const MEDIUM: ElevationToken = ElevationToken {
    level: ElevationLevel::Medium,
    offset_y: 6.0,
    blur_radius: 12.0,
    spread: 1.0,
    ambient_alpha: 0.06,
    key_alpha: 0.12,
};
/// The highest elevation: a modal or a command palette.
pub const HIGH: ElevationToken = ElevationToken {
    level: ElevationLevel::High,
    offset_y: 16.0,
    blur_radius: 32.0,
    spread: 2.0,
    ambient_alpha: 0.08,
    key_alpha: 0.20,
};

/// Every elevation step, from the resting one to the highest.
pub const ALL: [ElevationToken; 3] = [LOW, MEDIUM, HIGH];

/// Every elevation level, from the resting one to the highest.
pub const LEVELS: [ElevationLevel; 3] = ElevationLevel::ALL;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_elevation_ladder_rises() {
        assert!(LOW.blur_radius < MEDIUM.blur_radius);
        assert!(MEDIUM.blur_radius < HIGH.blur_radius);
        assert!(LOW.offset_y < MEDIUM.offset_y);
        assert!(MEDIUM.offset_y < HIGH.offset_y);
        assert!(LOW.key_alpha <= MEDIUM.key_alpha);
        assert!(MEDIUM.key_alpha <= HIGH.key_alpha);
        assert_eq!(ALL.len(), 3);
    }

    #[test]
    fn the_shadow_spec_splits_the_elevation_into_two_layers() {
        for token in ALL {
            let shadow = token.shadow();
            assert_eq!(shadow.key.offset_y, token.offset_y);
            assert_eq!(shadow.key.blur_radius, token.blur_radius);
            assert_eq!(shadow.key.alpha, token.key_alpha);
            assert_eq!(shadow.ambient.alpha, token.ambient_alpha);
            assert!(
                shadow.ambient.blur_radius > shadow.key.blur_radius,
                "the ambient layer must reach further than the key layer"
            );
            assert!(shadow.ambient.alpha < shadow.key.alpha);
        }
    }

    #[test]
    fn every_level_resolves_to_its_own_token() {
        assert_eq!(LEVELS.len(), 3);
        for level in ElevationLevel::ALL {
            let token = level.token();
            assert_eq!(token.level, level);
            assert_eq!(token.shadow().level, level);
            assert!(!level.label().is_empty());
        }
        assert_eq!(ElevationLevel::Low.token(), LOW);
        assert_eq!(ElevationLevel::Medium.token(), MEDIUM);
        assert_eq!(ElevationLevel::High.token(), HIGH);
    }

    #[test]
    fn the_ladder_keeps_one_token_per_level() {
        for (index, token) in ALL.iter().enumerate() {
            assert_eq!(token.level.index(), index);
            assert_eq!(token.level, ElevationLevel::ALL[index]);
        }
    }

    #[test]
    fn the_shadow_paints_the_ambient_layer_first() {
        for token in ALL {
            let shadow = token.shadow();
            let layers = shadow.layers();
            assert_eq!(layers.len(), 2);
            assert_eq!(layers[0], shadow.ambient);
            assert_eq!(layers[1], shadow.key);
        }
    }
}
