// (c) Copyright 2025 Mikołaj Kuranowski
// SPDX-License-Identifier: MIT

/// Mean radius of Earth, in kilometers.
/// Source: https://en.wikipedia.org/wiki/Earth_radius#Arithmetic_mean_radius
const EARTH_RADIUS: f64 = 6371.0088;

/// Mean diameter of Earth, in kilometers.
/// Source: https://en.wikipedia.org/wiki/Earth_radius#Arithmetic_mean_radius
const EARTH_DIAMETER: f64 = EARTH_RADIUS + EARTH_RADIUS;

/// Calculates the great-circle distance between two lat-lon positions
/// on Earth using the [haversine formula](https://en.wikipedia.org/wiki/Haversine_formula).
/// Returns the result in kilometers.
pub fn earth_distance(lat1: f32, lon1: f32, lat2: f32, lon2: f32) -> f32 {
    let lat1 = (lat1 as f64).to_radians();
    let lon1 = (lon1 as f64).to_radians();
    let lat2 = (lat2 as f64).to_radians();
    let lon2 = (lon2 as f64).to_radians();

    let sin_dlat_half = ((lat2 - lat1) * 0.5).sin();
    let sin_dlon_half = ((lon2 - lon1) * 0.5).sin();

    let h = sin_dlat_half * sin_dlat_half + lat1.cos() * lat2.cos() * sin_dlon_half * sin_dlon_half;

    (EARTH_DIAMETER * h.sqrt().asin()) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    const CENTRUM: (f32, f32) = (52.23024, 21.01062);
    const STADION: (f32, f32) = (52.23852, 21.0446);
    const FALENICA: (f32, f32) = (52.16125, 21.21147);

    #[test]
    fn centrum_stadion() {
        let d = earth_distance(CENTRUM.0, CENTRUM.1, STADION.0, STADION.1);
        assert_eq!(d, 2.49049);
    }

    #[test]
    fn centrum_falenica() {
        let d = earth_distance(CENTRUM.0, CENTRUM.1, FALENICA.0, FALENICA.1);
        assert_eq!(d, 15.692483);
    }
}
