//! Literal source table: field names say Lat/Lon, but source values are lon/lat.
//! Do not swap or repair coordinates (including RU -18.0) during the port.
use std::{collections::BTreeMap, sync::OnceLock};
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GeoBounds {
    pub min_lat: f64,
    pub min_lon: f64,
    pub max_lat: f64,
    pub max_lon: f64,
}
pub fn country_bounds() -> &'static BTreeMap<&'static str, GeoBounds> {
    static TABLE: OnceLock<BTreeMap<&'static str, GeoBounds>> = OnceLock::new();
    TABLE.get_or_init(|| {
        [
            (
                "AF",
                GeoBounds {
                    min_lat: 60.53,
                    min_lon: 29.32,
                    max_lat: 75.16,
                    max_lon: 38.49,
                },
            ),
            (
                "AO",
                GeoBounds {
                    min_lat: 11.64,
                    min_lon: -17.93,
                    max_lat: 24.08,
                    max_lon: -4.44,
                },
            ),
            (
                "AL",
                GeoBounds {
                    min_lat: 19.3,
                    min_lon: 39.62,
                    max_lat: 21.02,
                    max_lon: 42.69,
                },
            ),
            (
                "AE",
                GeoBounds {
                    min_lat: 51.58,
                    min_lon: 22.5,
                    max_lat: 56.4,
                    max_lon: 26.06,
                },
            ),
            (
                "AR",
                GeoBounds {
                    min_lat: -73.42,
                    min_lon: -55.25,
                    max_lat: -53.63,
                    max_lon: -21.83,
                },
            ),
            (
                "AM",
                GeoBounds {
                    min_lat: 43.58,
                    min_lon: 38.74,
                    max_lat: 46.51,
                    max_lon: 41.25,
                },
            ),
            (
                "AQ",
                GeoBounds {
                    min_lat: -180.0,
                    min_lon: -90.0,
                    max_lat: 180.0,
                    max_lon: -63.27,
                },
            ),
            (
                "TF",
                GeoBounds {
                    min_lat: 68.72,
                    min_lon: -49.78,
                    max_lat: 70.56,
                    max_lon: -48.63,
                },
            ),
            (
                "AU",
                GeoBounds {
                    min_lat: 113.34,
                    min_lon: -43.63,
                    max_lat: 153.57,
                    max_lon: -10.67,
                },
            ),
            (
                "AT",
                GeoBounds {
                    min_lat: 9.48,
                    min_lon: 46.43,
                    max_lat: 16.98,
                    max_lon: 49.04,
                },
            ),
            (
                "AZ",
                GeoBounds {
                    min_lat: 44.79,
                    min_lon: 38.27,
                    max_lat: 50.39,
                    max_lon: 41.86,
                },
            ),
            (
                "BI",
                GeoBounds {
                    min_lat: 29.02,
                    min_lon: -4.5,
                    max_lat: 30.75,
                    max_lon: -2.35,
                },
            ),
            (
                "BE",
                GeoBounds {
                    min_lat: 2.51,
                    min_lon: 49.53,
                    max_lat: 6.16,
                    max_lon: 51.48,
                },
            ),
            (
                "BJ",
                GeoBounds {
                    min_lat: 0.77,
                    min_lon: 6.14,
                    max_lat: 3.8,
                    max_lon: 12.24,
                },
            ),
            (
                "BF",
                GeoBounds {
                    min_lat: -5.47,
                    min_lon: 9.61,
                    max_lat: 2.18,
                    max_lon: 15.12,
                },
            ),
            (
                "BD",
                GeoBounds {
                    min_lat: 88.08,
                    min_lon: 20.67,
                    max_lat: 92.67,
                    max_lon: 26.45,
                },
            ),
            (
                "BG",
                GeoBounds {
                    min_lat: 22.38,
                    min_lon: 41.23,
                    max_lat: 28.56,
                    max_lon: 44.23,
                },
            ),
            (
                "BS",
                GeoBounds {
                    min_lat: -78.98,
                    min_lon: 23.71,
                    max_lat: -77.0,
                    max_lon: 27.04,
                },
            ),
            (
                "BA",
                GeoBounds {
                    min_lat: 15.75,
                    min_lon: 42.65,
                    max_lat: 19.6,
                    max_lon: 45.23,
                },
            ),
            (
                "BY",
                GeoBounds {
                    min_lat: 23.2,
                    min_lon: 51.32,
                    max_lat: 32.69,
                    max_lon: 56.17,
                },
            ),
            (
                "BZ",
                GeoBounds {
                    min_lat: -89.23,
                    min_lon: 15.89,
                    max_lat: -88.11,
                    max_lon: 18.5,
                },
            ),
            (
                "BO",
                GeoBounds {
                    min_lat: -69.59,
                    min_lon: -22.87,
                    max_lat: -57.5,
                    max_lon: -9.76,
                },
            ),
            (
                "BR",
                GeoBounds {
                    min_lat: -73.99,
                    min_lon: -33.77,
                    max_lat: -34.73,
                    max_lon: 5.24,
                },
            ),
            (
                "BN",
                GeoBounds {
                    min_lat: 114.2,
                    min_lon: 4.01,
                    max_lat: 115.45,
                    max_lon: 5.45,
                },
            ),
            (
                "BT",
                GeoBounds {
                    min_lat: 88.81,
                    min_lon: 26.72,
                    max_lat: 92.1,
                    max_lon: 28.3,
                },
            ),
            (
                "BW",
                GeoBounds {
                    min_lat: 19.9,
                    min_lon: -26.83,
                    max_lat: 29.43,
                    max_lon: -17.66,
                },
            ),
            (
                "CF",
                GeoBounds {
                    min_lat: 14.46,
                    min_lon: 2.27,
                    max_lat: 27.37,
                    max_lon: 11.14,
                },
            ),
            (
                "CA",
                GeoBounds {
                    min_lat: -141.0,
                    min_lon: 41.68,
                    max_lat: -52.65,
                    max_lon: 73.23,
                },
            ),
            (
                "CH",
                GeoBounds {
                    min_lat: 6.02,
                    min_lon: 45.78,
                    max_lat: 10.44,
                    max_lon: 47.83,
                },
            ),
            (
                "CL",
                GeoBounds {
                    min_lat: -75.64,
                    min_lon: -55.61,
                    max_lat: -66.96,
                    max_lon: -17.58,
                },
            ),
            (
                "CN",
                GeoBounds {
                    min_lat: 73.68,
                    min_lon: 18.2,
                    max_lat: 135.03,
                    max_lon: 53.46,
                },
            ),
            (
                "CI",
                GeoBounds {
                    min_lat: -8.6,
                    min_lon: 4.34,
                    max_lat: -2.56,
                    max_lon: 10.52,
                },
            ),
            (
                "CM",
                GeoBounds {
                    min_lat: 8.49,
                    min_lon: 1.73,
                    max_lat: 16.01,
                    max_lon: 12.86,
                },
            ),
            (
                "CD",
                GeoBounds {
                    min_lat: 12.18,
                    min_lon: -13.26,
                    max_lat: 31.17,
                    max_lon: 5.26,
                },
            ),
            (
                "CG",
                GeoBounds {
                    min_lat: 11.09,
                    min_lon: -5.04,
                    max_lat: 18.45,
                    max_lon: 3.73,
                },
            ),
            (
                "CO",
                GeoBounds {
                    min_lat: -78.99,
                    min_lon: -4.3,
                    max_lat: -66.88,
                    max_lon: 12.44,
                },
            ),
            (
                "CR",
                GeoBounds {
                    min_lat: -85.94,
                    min_lon: 8.23,
                    max_lat: -82.55,
                    max_lon: 11.22,
                },
            ),
            (
                "CU",
                GeoBounds {
                    min_lat: -84.97,
                    min_lon: 19.86,
                    max_lat: -74.18,
                    max_lon: 23.19,
                },
            ),
            (
                "CY",
                GeoBounds {
                    min_lat: 32.26,
                    min_lon: 34.57,
                    max_lat: 34.0,
                    max_lon: 35.17,
                },
            ),
            (
                "CZ",
                GeoBounds {
                    min_lat: 12.24,
                    min_lon: 48.56,
                    max_lat: 18.85,
                    max_lon: 51.12,
                },
            ),
            (
                "DE",
                GeoBounds {
                    min_lat: 5.99,
                    min_lon: 47.3,
                    max_lat: 15.02,
                    max_lon: 54.98,
                },
            ),
            (
                "DJ",
                GeoBounds {
                    min_lat: 41.66,
                    min_lon: 10.93,
                    max_lat: 43.32,
                    max_lon: 12.7,
                },
            ),
            (
                "DK",
                GeoBounds {
                    min_lat: 8.09,
                    min_lon: 54.8,
                    max_lat: 12.69,
                    max_lon: 57.73,
                },
            ),
            (
                "DO",
                GeoBounds {
                    min_lat: -71.95,
                    min_lon: 17.6,
                    max_lat: -68.32,
                    max_lon: 19.88,
                },
            ),
            (
                "DZ",
                GeoBounds {
                    min_lat: -8.68,
                    min_lon: 19.06,
                    max_lat: 12.0,
                    max_lon: 37.12,
                },
            ),
            (
                "EC",
                GeoBounds {
                    min_lat: -80.97,
                    min_lon: -4.96,
                    max_lat: -75.23,
                    max_lon: 1.38,
                },
            ),
            (
                "EG",
                GeoBounds {
                    min_lat: 24.7,
                    min_lon: 22.0,
                    max_lat: 36.87,
                    max_lon: 31.59,
                },
            ),
            (
                "ER",
                GeoBounds {
                    min_lat: 36.32,
                    min_lon: 12.46,
                    max_lat: 43.08,
                    max_lon: 18.0,
                },
            ),
            (
                "ES",
                GeoBounds {
                    min_lat: -9.39,
                    min_lon: 35.95,
                    max_lat: 3.04,
                    max_lon: 43.75,
                },
            ),
            (
                "EE",
                GeoBounds {
                    min_lat: 23.34,
                    min_lon: 57.47,
                    max_lat: 28.13,
                    max_lon: 59.61,
                },
            ),
            (
                "ET",
                GeoBounds {
                    min_lat: 32.95,
                    min_lon: 3.42,
                    max_lat: 47.79,
                    max_lon: 14.96,
                },
            ),
            (
                "FI",
                GeoBounds {
                    min_lat: 20.65,
                    min_lon: 59.85,
                    max_lat: 31.52,
                    max_lon: 70.16,
                },
            ),
            (
                "FJ",
                GeoBounds {
                    min_lat: -180.0,
                    min_lon: -18.29,
                    max_lat: 180.0,
                    max_lon: -16.02,
                },
            ),
            (
                "FK",
                GeoBounds {
                    min_lat: -61.2,
                    min_lon: -52.3,
                    max_lat: -57.75,
                    max_lon: -51.1,
                },
            ),
            (
                "FR",
                GeoBounds {
                    min_lat: -5.0,
                    min_lon: 42.5,
                    max_lat: 9.56,
                    max_lon: 51.15,
                },
            ),
            (
                "GA",
                GeoBounds {
                    min_lat: 8.8,
                    min_lon: -3.98,
                    max_lat: 14.43,
                    max_lon: 2.33,
                },
            ),
            (
                "GB",
                GeoBounds {
                    min_lat: -7.57,
                    min_lon: 49.96,
                    max_lat: 1.68,
                    max_lon: 58.64,
                },
            ),
            (
                "GE",
                GeoBounds {
                    min_lat: 39.96,
                    min_lon: 41.06,
                    max_lat: 46.64,
                    max_lon: 43.55,
                },
            ),
            (
                "GH",
                GeoBounds {
                    min_lat: -3.24,
                    min_lon: 4.71,
                    max_lat: 1.06,
                    max_lon: 11.1,
                },
            ),
            (
                "GN",
                GeoBounds {
                    min_lat: -15.13,
                    min_lon: 7.31,
                    max_lat: -7.83,
                    max_lon: 12.59,
                },
            ),
            (
                "GM",
                GeoBounds {
                    min_lat: -16.84,
                    min_lon: 13.13,
                    max_lat: -13.84,
                    max_lon: 13.88,
                },
            ),
            (
                "GW",
                GeoBounds {
                    min_lat: -16.68,
                    min_lon: 11.04,
                    max_lat: -13.7,
                    max_lon: 12.63,
                },
            ),
            (
                "GQ",
                GeoBounds {
                    min_lat: 9.31,
                    min_lon: 1.01,
                    max_lat: 11.29,
                    max_lon: 2.28,
                },
            ),
            (
                "GR",
                GeoBounds {
                    min_lat: 20.15,
                    min_lon: 34.92,
                    max_lat: 26.6,
                    max_lon: 41.83,
                },
            ),
            (
                "GL",
                GeoBounds {
                    min_lat: -73.3,
                    min_lon: 60.04,
                    max_lat: -12.21,
                    max_lon: 83.65,
                },
            ),
            (
                "GT",
                GeoBounds {
                    min_lat: -92.23,
                    min_lon: 13.74,
                    max_lat: -88.23,
                    max_lon: 17.82,
                },
            ),
            (
                "GY",
                GeoBounds {
                    min_lat: -61.41,
                    min_lon: 1.27,
                    max_lat: -56.54,
                    max_lon: 8.37,
                },
            ),
            (
                "HN",
                GeoBounds {
                    min_lat: -89.35,
                    min_lon: 12.98,
                    max_lat: -83.15,
                    max_lon: 16.01,
                },
            ),
            (
                "HR",
                GeoBounds {
                    min_lat: 13.66,
                    min_lon: 42.48,
                    max_lat: 19.39,
                    max_lon: 46.5,
                },
            ),
            (
                "HT",
                GeoBounds {
                    min_lat: -74.46,
                    min_lon: 18.03,
                    max_lat: -71.62,
                    max_lon: 19.92,
                },
            ),
            (
                "HU",
                GeoBounds {
                    min_lat: 16.2,
                    min_lon: 45.76,
                    max_lat: 22.71,
                    max_lon: 48.62,
                },
            ),
            (
                "ID",
                GeoBounds {
                    min_lat: 95.29,
                    min_lon: -10.36,
                    max_lat: 141.03,
                    max_lon: 5.48,
                },
            ),
            (
                "IN",
                GeoBounds {
                    min_lat: 68.18,
                    min_lon: 7.97,
                    max_lat: 97.4,
                    max_lon: 35.49,
                },
            ),
            (
                "IE",
                GeoBounds {
                    min_lat: -9.98,
                    min_lon: 51.67,
                    max_lat: -6.03,
                    max_lon: 55.13,
                },
            ),
            (
                "IR",
                GeoBounds {
                    min_lat: 44.11,
                    min_lon: 25.08,
                    max_lat: 63.32,
                    max_lon: 39.71,
                },
            ),
            (
                "IQ",
                GeoBounds {
                    min_lat: 38.79,
                    min_lon: 29.1,
                    max_lat: 48.57,
                    max_lon: 37.39,
                },
            ),
            (
                "IS",
                GeoBounds {
                    min_lat: -24.33,
                    min_lon: 63.5,
                    max_lat: -13.61,
                    max_lon: 66.53,
                },
            ),
            (
                "IL",
                GeoBounds {
                    min_lat: 34.27,
                    min_lon: 29.5,
                    max_lat: 35.84,
                    max_lon: 33.28,
                },
            ),
            (
                "IT",
                GeoBounds {
                    min_lat: 6.75,
                    min_lon: 36.62,
                    max_lat: 18.48,
                    max_lon: 47.12,
                },
            ),
            (
                "JM",
                GeoBounds {
                    min_lat: -78.34,
                    min_lon: 17.7,
                    max_lat: -76.2,
                    max_lon: 18.52,
                },
            ),
            (
                "JO",
                GeoBounds {
                    min_lat: 34.92,
                    min_lon: 29.2,
                    max_lat: 39.2,
                    max_lon: 33.38,
                },
            ),
            (
                "JP",
                GeoBounds {
                    min_lat: 129.41,
                    min_lon: 31.03,
                    max_lat: 145.54,
                    max_lon: 45.55,
                },
            ),
            (
                "KZ",
                GeoBounds {
                    min_lat: 46.47,
                    min_lon: 40.66,
                    max_lat: 87.36,
                    max_lon: 55.39,
                },
            ),
            (
                "KE",
                GeoBounds {
                    min_lat: 33.89,
                    min_lon: -4.68,
                    max_lat: 41.86,
                    max_lon: 5.51,
                },
            ),
            (
                "KG",
                GeoBounds {
                    min_lat: 69.46,
                    min_lon: 39.28,
                    max_lat: 80.26,
                    max_lon: 43.3,
                },
            ),
            (
                "KH",
                GeoBounds {
                    min_lat: 102.35,
                    min_lon: 10.49,
                    max_lat: 107.61,
                    max_lon: 14.57,
                },
            ),
            (
                "KR",
                GeoBounds {
                    min_lat: 126.12,
                    min_lon: 34.39,
                    max_lat: 129.47,
                    max_lon: 38.61,
                },
            ),
            (
                "KW",
                GeoBounds {
                    min_lat: 46.57,
                    min_lon: 28.53,
                    max_lat: 48.42,
                    max_lon: 30.06,
                },
            ),
            (
                "LA",
                GeoBounds {
                    min_lat: 100.12,
                    min_lon: 13.88,
                    max_lat: 107.56,
                    max_lon: 22.46,
                },
            ),
            (
                "LB",
                GeoBounds {
                    min_lat: 35.13,
                    min_lon: 33.09,
                    max_lat: 36.61,
                    max_lon: 34.64,
                },
            ),
            (
                "LR",
                GeoBounds {
                    min_lat: -11.44,
                    min_lon: 4.36,
                    max_lat: -7.54,
                    max_lon: 8.54,
                },
            ),
            (
                "LY",
                GeoBounds {
                    min_lat: 9.32,
                    min_lon: 19.58,
                    max_lat: 25.16,
                    max_lon: 33.14,
                },
            ),
            (
                "LK",
                GeoBounds {
                    min_lat: 79.7,
                    min_lon: 5.97,
                    max_lat: 81.79,
                    max_lon: 9.82,
                },
            ),
            (
                "LS",
                GeoBounds {
                    min_lat: 27.0,
                    min_lon: -30.65,
                    max_lat: 29.33,
                    max_lon: -28.65,
                },
            ),
            (
                "LT",
                GeoBounds {
                    min_lat: 21.06,
                    min_lon: 53.91,
                    max_lat: 26.59,
                    max_lon: 56.37,
                },
            ),
            (
                "LU",
                GeoBounds {
                    min_lat: 5.67,
                    min_lon: 49.44,
                    max_lat: 6.24,
                    max_lon: 50.13,
                },
            ),
            (
                "LV",
                GeoBounds {
                    min_lat: 21.06,
                    min_lon: 55.62,
                    max_lat: 28.18,
                    max_lon: 57.97,
                },
            ),
            (
                "MA",
                GeoBounds {
                    min_lat: -17.02,
                    min_lon: 21.42,
                    max_lat: -1.12,
                    max_lon: 35.76,
                },
            ),
            (
                "MD",
                GeoBounds {
                    min_lat: 26.62,
                    min_lon: 45.49,
                    max_lat: 30.02,
                    max_lon: 48.47,
                },
            ),
            (
                "MG",
                GeoBounds {
                    min_lat: 43.25,
                    min_lon: -25.6,
                    max_lat: 50.48,
                    max_lon: -12.04,
                },
            ),
            (
                "MX",
                GeoBounds {
                    min_lat: -117.13,
                    min_lon: 14.54,
                    max_lat: -86.81,
                    max_lon: 32.72,
                },
            ),
            (
                "MK",
                GeoBounds {
                    min_lat: 20.46,
                    min_lon: 40.84,
                    max_lat: 22.95,
                    max_lon: 42.32,
                },
            ),
            (
                "ML",
                GeoBounds {
                    min_lat: -12.17,
                    min_lon: 10.1,
                    max_lat: 4.27,
                    max_lon: 24.97,
                },
            ),
            (
                "MM",
                GeoBounds {
                    min_lat: 92.3,
                    min_lon: 9.93,
                    max_lat: 101.18,
                    max_lon: 28.34,
                },
            ),
            (
                "ME",
                GeoBounds {
                    min_lat: 18.45,
                    min_lon: 41.88,
                    max_lat: 20.34,
                    max_lon: 43.52,
                },
            ),
            (
                "MN",
                GeoBounds {
                    min_lat: 87.75,
                    min_lon: 41.6,
                    max_lat: 119.77,
                    max_lon: 52.05,
                },
            ),
            (
                "MZ",
                GeoBounds {
                    min_lat: 30.18,
                    min_lon: -26.74,
                    max_lat: 40.78,
                    max_lon: -10.32,
                },
            ),
            (
                "MR",
                GeoBounds {
                    min_lat: -17.06,
                    min_lon: 14.62,
                    max_lat: -4.92,
                    max_lon: 27.4,
                },
            ),
            (
                "MW",
                GeoBounds {
                    min_lat: 32.69,
                    min_lon: -16.8,
                    max_lat: 35.77,
                    max_lon: -9.23,
                },
            ),
            (
                "MY",
                GeoBounds {
                    min_lat: 100.09,
                    min_lon: 0.77,
                    max_lat: 119.18,
                    max_lon: 6.93,
                },
            ),
            (
                "NA",
                GeoBounds {
                    min_lat: 11.73,
                    min_lon: -29.05,
                    max_lat: 25.08,
                    max_lon: -16.94,
                },
            ),
            (
                "NC",
                GeoBounds {
                    min_lat: 164.03,
                    min_lon: -22.4,
                    max_lat: 167.12,
                    max_lon: -20.11,
                },
            ),
            (
                "NE",
                GeoBounds {
                    min_lat: 0.3,
                    min_lon: 11.66,
                    max_lat: 15.9,
                    max_lon: 23.47,
                },
            ),
            (
                "NG",
                GeoBounds {
                    min_lat: 2.69,
                    min_lon: 4.24,
                    max_lat: 14.58,
                    max_lon: 13.87,
                },
            ),
            (
                "NI",
                GeoBounds {
                    min_lat: -87.67,
                    min_lon: 10.73,
                    max_lat: -83.15,
                    max_lon: 15.02,
                },
            ),
            (
                "NL",
                GeoBounds {
                    min_lat: 3.31,
                    min_lon: 50.8,
                    max_lat: 7.09,
                    max_lon: 53.51,
                },
            ),
            (
                "NO",
                GeoBounds {
                    min_lat: 4.99,
                    min_lon: 58.08,
                    max_lat: 31.29,
                    max_lon: 70.92,
                },
            ),
            (
                "NP",
                GeoBounds {
                    min_lat: 80.09,
                    min_lon: 26.4,
                    max_lat: 88.17,
                    max_lon: 30.42,
                },
            ),
            (
                "NZ",
                GeoBounds {
                    min_lat: 166.51,
                    min_lon: -46.64,
                    max_lat: 178.52,
                    max_lon: -34.45,
                },
            ),
            (
                "OM",
                GeoBounds {
                    min_lat: 52.0,
                    min_lon: 16.65,
                    max_lat: 59.81,
                    max_lon: 26.4,
                },
            ),
            (
                "PK",
                GeoBounds {
                    min_lat: 60.87,
                    min_lon: 23.69,
                    max_lat: 77.84,
                    max_lon: 37.13,
                },
            ),
            (
                "PA",
                GeoBounds {
                    min_lat: -82.97,
                    min_lon: 7.22,
                    max_lat: -77.24,
                    max_lon: 9.61,
                },
            ),
            (
                "PE",
                GeoBounds {
                    min_lat: -81.41,
                    min_lon: -18.35,
                    max_lat: -68.67,
                    max_lon: -0.06,
                },
            ),
            (
                "PH",
                GeoBounds {
                    min_lat: 117.17,
                    min_lon: 5.58,
                    max_lat: 126.54,
                    max_lon: 18.51,
                },
            ),
            (
                "PG",
                GeoBounds {
                    min_lat: 141.0,
                    min_lon: -10.65,
                    max_lat: 156.02,
                    max_lon: -2.5,
                },
            ),
            (
                "PL",
                GeoBounds {
                    min_lat: 14.07,
                    min_lon: 49.03,
                    max_lat: 24.03,
                    max_lon: 54.85,
                },
            ),
            (
                "PR",
                GeoBounds {
                    min_lat: -67.24,
                    min_lon: 17.95,
                    max_lat: -65.59,
                    max_lon: 18.52,
                },
            ),
            (
                "KP",
                GeoBounds {
                    min_lat: 124.27,
                    min_lon: 37.67,
                    max_lat: 130.78,
                    max_lon: 42.99,
                },
            ),
            (
                "PT",
                GeoBounds {
                    min_lat: -9.53,
                    min_lon: 36.84,
                    max_lat: -6.39,
                    max_lon: 42.28,
                },
            ),
            (
                "PY",
                GeoBounds {
                    min_lat: -62.69,
                    min_lon: -27.55,
                    max_lat: -54.29,
                    max_lon: -19.34,
                },
            ),
            (
                "QA",
                GeoBounds {
                    min_lat: 50.74,
                    min_lon: 24.56,
                    max_lat: 51.61,
                    max_lon: 26.11,
                },
            ),
            (
                "RO",
                GeoBounds {
                    min_lat: 20.22,
                    min_lon: 43.69,
                    max_lat: 29.63,
                    max_lon: 48.22,
                },
            ),
            (
                "RU",
                GeoBounds {
                    min_lat: -18.0,
                    min_lon: 41.15,
                    max_lat: 180.0,
                    max_lon: 81.25,
                },
            ),
            (
                "RW",
                GeoBounds {
                    min_lat: 29.02,
                    min_lon: -2.92,
                    max_lat: 30.82,
                    max_lon: -1.13,
                },
            ),
            (
                "SA",
                GeoBounds {
                    min_lat: 34.63,
                    min_lon: 16.35,
                    max_lat: 55.67,
                    max_lon: 32.16,
                },
            ),
            (
                "SD",
                GeoBounds {
                    min_lat: 21.94,
                    min_lon: 8.62,
                    max_lat: 38.41,
                    max_lon: 22.0,
                },
            ),
            (
                "SS",
                GeoBounds {
                    min_lat: 23.89,
                    min_lon: 3.51,
                    max_lat: 35.3,
                    max_lon: 12.25,
                },
            ),
            (
                "SN",
                GeoBounds {
                    min_lat: -17.63,
                    min_lon: 12.33,
                    max_lat: -11.47,
                    max_lon: 16.6,
                },
            ),
            (
                "SB",
                GeoBounds {
                    min_lat: 156.49,
                    min_lon: -10.83,
                    max_lat: 162.4,
                    max_lon: -6.6,
                },
            ),
            (
                "SL",
                GeoBounds {
                    min_lat: -13.25,
                    min_lon: 6.79,
                    max_lat: -10.23,
                    max_lon: 10.05,
                },
            ),
            (
                "SV",
                GeoBounds {
                    min_lat: -90.1,
                    min_lon: 13.15,
                    max_lat: -87.72,
                    max_lon: 14.42,
                },
            ),
            (
                "SO",
                GeoBounds {
                    min_lat: 40.98,
                    min_lon: -1.68,
                    max_lat: 51.13,
                    max_lon: 12.02,
                },
            ),
            (
                "RS",
                GeoBounds {
                    min_lat: 18.83,
                    min_lon: 42.25,
                    max_lat: 22.99,
                    max_lon: 46.17,
                },
            ),
            (
                "SR",
                GeoBounds {
                    min_lat: -58.04,
                    min_lon: 1.82,
                    max_lat: -53.96,
                    max_lon: 6.03,
                },
            ),
            (
                "SK",
                GeoBounds {
                    min_lat: 16.88,
                    min_lon: 47.76,
                    max_lat: 22.56,
                    max_lon: 49.57,
                },
            ),
            (
                "SI",
                GeoBounds {
                    min_lat: 13.7,
                    min_lon: 45.45,
                    max_lat: 16.56,
                    max_lon: 46.85,
                },
            ),
            (
                "SE",
                GeoBounds {
                    min_lat: 11.03,
                    min_lon: 55.36,
                    max_lat: 23.9,
                    max_lon: 69.11,
                },
            ),
            (
                "SZ",
                GeoBounds {
                    min_lat: 30.68,
                    min_lon: -27.29,
                    max_lat: 32.07,
                    max_lon: -25.66,
                },
            ),
            (
                "SY",
                GeoBounds {
                    min_lat: 35.7,
                    min_lon: 32.31,
                    max_lat: 42.35,
                    max_lon: 37.23,
                },
            ),
            (
                "TD",
                GeoBounds {
                    min_lat: 13.54,
                    min_lon: 7.42,
                    max_lat: 23.89,
                    max_lon: 23.41,
                },
            ),
            (
                "TG",
                GeoBounds {
                    min_lat: -0.05,
                    min_lon: 5.93,
                    max_lat: 1.87,
                    max_lon: 11.02,
                },
            ),
            (
                "TH",
                GeoBounds {
                    min_lat: 97.38,
                    min_lon: 5.69,
                    max_lat: 105.59,
                    max_lon: 20.42,
                },
            ),
            (
                "TJ",
                GeoBounds {
                    min_lat: 67.44,
                    min_lon: 36.74,
                    max_lat: 74.98,
                    max_lon: 40.96,
                },
            ),
            (
                "TM",
                GeoBounds {
                    min_lat: 52.5,
                    min_lon: 35.27,
                    max_lat: 66.55,
                    max_lon: 42.75,
                },
            ),
            (
                "TL",
                GeoBounds {
                    min_lat: 124.97,
                    min_lon: -9.39,
                    max_lat: 127.34,
                    max_lon: -8.27,
                },
            ),
            (
                "TT",
                GeoBounds {
                    min_lat: -61.95,
                    min_lon: 10.0,
                    max_lat: -60.9,
                    max_lon: 10.89,
                },
            ),
            (
                "TN",
                GeoBounds {
                    min_lat: 7.52,
                    min_lon: 30.31,
                    max_lat: 11.49,
                    max_lon: 37.35,
                },
            ),
            (
                "TR",
                GeoBounds {
                    min_lat: 26.04,
                    min_lon: 35.82,
                    max_lat: 44.79,
                    max_lon: 42.14,
                },
            ),
            (
                "TW",
                GeoBounds {
                    min_lat: 120.11,
                    min_lon: 21.97,
                    max_lat: 121.95,
                    max_lon: 25.3,
                },
            ),
            (
                "TZ",
                GeoBounds {
                    min_lat: 29.34,
                    min_lon: -11.72,
                    max_lat: 40.32,
                    max_lon: -0.95,
                },
            ),
            (
                "UG",
                GeoBounds {
                    min_lat: 29.58,
                    min_lon: -1.44,
                    max_lat: 35.04,
                    max_lon: 4.25,
                },
            ),
            (
                "UA",
                GeoBounds {
                    min_lat: 22.09,
                    min_lon: 44.36,
                    max_lat: 40.08,
                    max_lon: 52.34,
                },
            ),
            (
                "UY",
                GeoBounds {
                    min_lat: -58.43,
                    min_lon: -34.95,
                    max_lat: -53.21,
                    max_lon: -30.11,
                },
            ),
            (
                "US",
                GeoBounds {
                    min_lat: -125.0,
                    min_lon: 25.0,
                    max_lat: -66.96,
                    max_lon: 49.5,
                },
            ),
            (
                "UZ",
                GeoBounds {
                    min_lat: 55.93,
                    min_lon: 37.14,
                    max_lat: 73.06,
                    max_lon: 45.59,
                },
            ),
            (
                "VE",
                GeoBounds {
                    min_lat: -73.3,
                    min_lon: 0.72,
                    max_lat: -59.76,
                    max_lon: 12.16,
                },
            ),
            (
                "VN",
                GeoBounds {
                    min_lat: 102.17,
                    min_lon: 8.6,
                    max_lat: 109.34,
                    max_lon: 23.35,
                },
            ),
            (
                "VU",
                GeoBounds {
                    min_lat: 166.63,
                    min_lon: -16.6,
                    max_lat: 167.84,
                    max_lon: -14.63,
                },
            ),
            (
                "PS",
                GeoBounds {
                    min_lat: 34.93,
                    min_lon: 31.35,
                    max_lat: 35.55,
                    max_lon: 32.53,
                },
            ),
            (
                "YE",
                GeoBounds {
                    min_lat: 42.6,
                    min_lon: 12.59,
                    max_lat: 53.11,
                    max_lon: 19.0,
                },
            ),
            (
                "ZA",
                GeoBounds {
                    min_lat: 16.34,
                    min_lon: -34.82,
                    max_lat: 32.83,
                    max_lon: -22.09,
                },
            ),
            (
                "ZM",
                GeoBounds {
                    min_lat: 21.89,
                    min_lon: -17.96,
                    max_lat: 33.49,
                    max_lon: -8.24,
                },
            ),
            (
                "ZW",
                GeoBounds {
                    min_lat: 25.26,
                    min_lon: -22.27,
                    max_lat: 32.85,
                    max_lon: -15.51,
                },
            ),
        ]
        .into_iter()
        .collect()
    })
}
