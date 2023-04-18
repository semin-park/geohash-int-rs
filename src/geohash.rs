use std::collections::HashMap;

use crate::bits::{deinterleave64, interleave64};
use std::ops::Range;

const LAT_MIN: f32 = -90f32;
const LAT_MAX: f32 = 90f32;
const LNG_MIN: f32 = -180f32;
const LNG_MAX: f32 = 180f32;

const LAT_RNG: Range<f32> = Range {
    start: LAT_MIN,
    end: LAT_MAX,
};
const LNG_RNG: Range<f32> = Range {
    start: LNG_MIN,
    end: LNG_MAX,
};

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    North,
    East,
    South,
    West,
    NorthEast,
    SouthEast,
    SouthWest,
    NorthWest,
}

#[derive(PartialEq, Debug)]
pub struct Coord {
    latitude: f32,
    longitude: f32,
}

impl Coord {
    pub fn new(latitude: f32, longitude: f32) -> Self {
        if !LAT_RNG.contains(&latitude) {
            panic!("latitude must be in ({}, {}).", LAT_RNG.start, LAT_RNG.end);
        }
        if !LNG_RNG.contains(&longitude) {
            panic!("longitude must be in ({}, {}).", LNG_RNG.start, LNG_RNG.end);
        }
        Coord {
            latitude,
            longitude,
        }
    }

    /// Computes the L2 distance, also known as the Euclidean distance.
    pub fn distance(&self, coord: &Coord) -> f32 {
        let lat_diff = self.latitude - coord.latitude;
        let lng_diff = self.longitude - coord.longitude;
        (lat_diff.powi(2) + lng_diff.powi(2)).sqrt()
    }
}

pub trait RangeExtension {
    type Idx;

    fn length(&self) -> Self::Idx;

    fn center(&self) -> Self::Idx;
}

impl RangeExtension for Range<f32> {
    type Idx = f32;

    fn length(&self) -> f32 {
        self.end - self.start
    }

    fn center(&self) -> f32 {
        (self.start + self.end) / 2f32
    }
}

pub struct Area {
    lat_range: Range<f32>,
    lng_range: Range<f32>,
}

impl Area {
    pub fn center(&self) -> Coord {
        Coord {
            latitude: self.lat_range.center(),
            longitude: self.lng_range.center(),
        }
    }

    pub fn contains(&self, coord: &Coord) -> bool {
        self.lat_range.contains(&coord.latitude) && self.lng_range.contains(&coord.longitude)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct GeoBits {
    bits: u64,
    precision: u8,
}

pub type Neighbors = HashMap<Direction, GeoBits>;

const LAT_BITS: u64 = 0x5555555555555555;
const LNG_BITS: u64 = 0xAAAAAAAAAAAAAAAA;

impl GeoBits {
    pub fn from(coord: &Coord, precision: u8) -> Self {
        if precision == 0 || precision > 32 {
            panic!("Precision should satisfy 1 <= precision <= 32");
        }
        // Scale the coordinates to be between 0 and 1
        let lat = (coord.latitude - LAT_MIN) / LAT_RNG.length();
        let lng = (coord.longitude - LNG_MIN) / LNG_RNG.length();

        // Change the representation of these floats to fixed point. Since
        // precision can be 32, we need u64.
        let lat = (lat as f64) * ((1u64 << precision) as f64);
        let lng = (lng as f64) * ((1u64 << precision) as f64);

        // Now we have pure bits that we can interleave.
        let lat = lat as u32;
        let lng = lng as u32;

        // Raw representation of geohash. Users can group every 5 bits and store
        // them as a hexadecimal string to implement the standard geohash.
        let bits: u64 = interleave64(lat, lng);

        GeoBits { bits, precision }
    }

    fn move_x(&mut self, left: bool) -> &mut Self {
        let mut lng = self.bits & LNG_BITS;
        let lat = self.bits & LAT_BITS;

        let num_unused_bits = 64 - self.precision * 2;
        let tmp = LAT_BITS >> num_unused_bits;
        if left {
            lng |= tmp;
            lng -= tmp + 1;
        } else {
            lng += tmp + 1;
        }
        lng &= LNG_BITS >> num_unused_bits;
        self.bits = lng | lat;
        self
    }

    fn move_y(&mut self, bottom: bool) -> &mut Self {
        let lng = self.bits & LNG_BITS;
        let mut lat = self.bits & LAT_BITS;

        let num_unused_bits = 64 - self.precision * 2;
        let tmp = LNG_BITS >> num_unused_bits;
        if bottom {
            lat |= tmp;
            lat -= tmp + 1;
        } else {
            lat += tmp + 1;
        }
        lat &= LAT_BITS >> num_unused_bits;
        self.bits = lng | lat;
        self
    }

    pub fn get_neighbors(&self) -> Neighbors {
        Neighbors::from([
            (Direction::North, self.get_neighbor(Direction::North)),
            (Direction::East, self.get_neighbor(Direction::East)),
            (Direction::South, self.get_neighbor(Direction::South)),
            (Direction::West, self.get_neighbor(Direction::West)),
            (
                Direction::NorthEast,
                self.get_neighbor(Direction::NorthEast),
            ),
            (
                Direction::SouthEast,
                self.get_neighbor(Direction::SouthEast),
            ),
            (
                Direction::SouthWest,
                self.get_neighbor(Direction::SouthWest),
            ),
            (
                Direction::NorthWest,
                self.get_neighbor(Direction::NorthWest),
            ),
        ])
    }

    pub fn get_neighbor(&self, direction: Direction) -> GeoBits {
        let mut bits = GeoBits {
            bits: self.bits,
            precision: self.precision,
        };
        match direction {
            Direction::North => bits.move_y(false),
            Direction::East => bits.move_x(false),
            Direction::South => bits.move_y(true),
            Direction::West => bits.move_x(true),
            Direction::NorthEast => bits.move_y(false).move_x(false),
            Direction::SouthEast => bits.move_y(true).move_x(false),
            Direction::SouthWest => bits.move_y(true).move_x(true),
            Direction::NorthWest => bits.move_y(false).move_x(true),
        };
        bits
    }

    pub fn next_leftbottom(&self) -> GeoBits {
        GeoBits {
            bits: self.bits << 2,
            precision: self.precision + 1,
        }
    }

    pub fn next_rightbottom(&self) -> GeoBits {
        GeoBits {
            bits: (self.bits << 2) + 2,
            precision: self.precision + 1,
        }
    }

    pub fn next_lefttop(&self) -> GeoBits {
        GeoBits {
            bits: (self.bits << 2) + 1,
            precision: self.precision + 1,
        }
    }

    pub fn next_righttop(&self) -> GeoBits {
        GeoBits {
            bits: (self.bits << 2) + 3,
            precision: self.precision + 1,
        }
    }
}

impl Into<Area> for GeoBits {
    fn into(self) -> Area {
        let (lng, lat) = deinterleave64(self.bits);

        let lat_scale = 180f32;
        let lng_scale = 360f32;

        // Note that if we look at the latitude and longitude bits separately,
        // each cell is +1 from the previous cell:
        //
        // |---------------|---------------|
        //         0               1
        // |-------|-------|-------|-------|
        //     00      01      10      11
        // |---|---|---|---|---|---|---|---|
        //  000 001 010 011 100 101 110 111
        //
        // Thus, to get the upper bound of a geohash, you just need to +1 to the
        // latitude bits and then convert the number back to floating point.
        let float_scale = (1u32 << self.precision) as f32;
        let lat_range = Range {
            start: LAT_MIN + (lat as f32 / float_scale) * lat_scale,
            end: LAT_MIN + ((lat + 1) as f32 / float_scale) * lat_scale,
        };
        let lng_range = Range {
            start: LNG_MIN + (lng as f32 / float_scale) * lng_scale,
            end: LNG_MIN + ((lng + 1) as f32 / float_scale) * lng_scale,
        };
        Area {
            lat_range,
            lng_range,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode() {
        let coord = Coord {
            latitude: 25.006,
            longitude: 121.46,
        };
        let hash = GeoBits::from(&coord, 15);
        assert_eq!(hash.bits, 0b111001100010110101100011101010);
        assert_eq!(hash.precision, 15);
    }

    #[test]
    fn decode() {
        let area: Area = GeoBits {
            bits: 0b111001100010110101100011101010,
            precision: 15,
        }
        .into();
        assert!(area.contains(&Coord {
            latitude: 25.006,
            longitude: 121.46,
        }));
    }

    #[test]
    fn next() {
        let hash = GeoBits {
            bits: 0b111001100010110101100011101010,
            precision: 15,
        };
        assert_eq!(
            hash.next_leftbottom(),
            GeoBits {
                bits: 0b11100110001011010110001110101000,
                precision: 16,
            }
        );
        assert_eq!(
            hash.next_lefttop(),
            GeoBits {
                bits: 0b11100110001011010110001110101001,
                precision: 16,
            }
        );
        assert_eq!(
            hash.next_rightbottom(),
            GeoBits {
                bits: 0b11100110001011010110001110101010,
                precision: 16,
            }
        );
        assert_eq!(
            hash.next_righttop(),
            GeoBits {
                bits: 0b11100110001011010110001110101011,
                precision: 16,
            }
        );
    }

    #[test]
    fn neighbor() {
        let hash = GeoBits {
            bits: 0b111001100010110101100011101010,
            precision: 15,
        };
        assert_eq!(hash.get_neighbors(), Neighbors::new());
    }
}
