use tg_core::{
    data::{data_statistics_chart::*, raw::raw_countries_bounds::*},
    statistics::{
        segment_tree::SegmentTree, statistics_data_deserialize::statistical_chart_from_json,
    },
};
fn float(s: &str) -> f64 {
    match s {
        "nan" | "-nan" => f64::NAN,
        _ => s.parse().unwrap(),
    }
}
fn floats(s: &str) -> Vec<f64> {
    s.split(';').filter(|v| !v.is_empty()).map(float).collect()
}
fn same(a: f64, b: f64) {
    assert!(
        a.to_bits() == b.to_bits() || a.is_nan() && b.is_nan(),
        "{a:?} != {b:?}"
    );
}
fn bytes(s: &str) -> String {
    String::from_utf8(
        s.as_bytes()
            .chunks_exact(2)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
            .collect(),
    )
    .unwrap()
}
#[test]
fn native_countries_chart_parser_indices_and_segment_ranges() {
    let json = include_str!("fixtures/data-calls-notify/inputs.jsonl")
        .lines()
        .collect::<Vec<_>>();
    let mut chart = StatisticalChart::default();
    let mut countries = 0;
    let mut part_cases = 0;
    let mut lines = 0;
    for row in include_str!("fixtures/data-calls-notify/native.csv").lines() {
        let v = row.split(',').collect::<Vec<_>>();
        match v[0] {
            "country" => {
                countries += 1;
                let b = country_bounds().get(v[1]).unwrap();
                for (a, b) in [b.min_lat, b.min_lon, b.max_lat, b.max_lon]
                    .into_iter()
                    .zip(v[2..].iter().map(|s| float(s)))
                {
                    same(a, b);
                }
            }
            "index" => {
                let n = v[1].parse::<usize>().unwrap();
                let mut c = StatisticalChart {
                    x: (0..n).map(|i| (i * 1000) as f64).collect(),
                    time_step: 1000.,
                    ..Default::default()
                };
                c.measure_data();
                let x = float(v[2]);
                assert_eq!(c.find_start_index(x), v[3].parse::<i32>().unwrap(), "{row}");
                assert_eq!(
                    c.find_end_index(0, x),
                    v[4].parse::<i32>().unwrap(),
                    "{row}"
                );
                if n > 0 {
                    assert_eq!(
                        c.find_index(0, n as i32 - 1, x),
                        v[5].parse::<i32>().unwrap(),
                        "{row}"
                    );
                }
            }
            "tree" => {
                let n = v[1].parse::<i64>().unwrap();
                let mut t = SegmentTree::new((0..n).map(|i| -i - 1).collect());
                let from = v[2].parse().unwrap();
                let to = v[3].parse().unwrap();
                assert_eq!(t.r_min_q(from, to), v[4].parse::<i64>().unwrap(), "{row}");
                assert_eq!(t.r_max_q(from, to), v[5].parse::<i64>().unwrap(), "{row}");
            }
            "parts" => {
                part_cases += 1;
                let n = v[1].parse::<u32>().unwrap();
                let parts = (0..n)
                    .map(|i| (i * 131072, vec![i as u8, 0, 255]))
                    .collect();
                let bytes = v[2]
                    .as_bytes()
                    .chunks_exact(2)
                    .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(
                    tg_core::data::data_media_preload::serialize_complex_parts_map(&parts),
                    bytes
                );
            }
            "json" => {
                chart =
                    statistical_chart_from_json(json[v[1].parse::<usize>().unwrap()].as_bytes());
                lines = 0;
                assert_eq!(chart.lines.len(), v[2].parse::<usize>().unwrap(), "{row}");
                assert_eq!(chart.max_value, v[3].parse::<i64>().unwrap(), "{row}");
                assert_eq!(chart.min_value, v[4].parse::<i64>().unwrap(), "{row}");
                same(chart.time_step, float(v[5]));
                same(chart.one_day_percentage, float(v[6]));
                assert_eq!(chart.is_footer_hidden, v[7] == "1");
                assert_eq!(chart.has_percentages, v[8] == "1");
                assert_eq!(chart.week_format, v[9] == "1");
                assert_eq!(chart.currency as i32, v[10].parse::<i32>().unwrap());
                same(chart.default_zoom_x_index.min, float(v[11]));
                same(chart.default_zoom_x_index.max, float(v[12]));
                assert_eq!(chart.x, floats(v[13]));
                let expected = floats(v[14]);
                assert_eq!(chart.x_percentage.len(), expected.len());
                for (a, b) in chart.x_percentage.iter().copied().zip(expected) {
                    same(a, b);
                }
            }
            "line" => {
                let l = &chart.lines[lines];
                lines += 1;
                assert_eq!(l.id, v[1].parse::<i32>().unwrap());
                assert_eq!(l.id_string, bytes(v[2]));
                assert_eq!(l.name, bytes(v[3]));
                assert_eq!(l.color_key, bytes(v[4]));
                assert_eq!(l.max_value, v[5].parse::<i64>().unwrap());
                assert_eq!(l.min_value, v[6].parse::<i64>().unwrap());
                assert_eq!(l.is_hidden_on_start, v[7] == "1");
                assert_eq!(
                    l.y,
                    v[8].split(';')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.parse::<i64>().unwrap())
                        .collect::<Vec<_>>()
                );
            }
            _ => panic!("{row}"),
        }
    }
    assert_eq!(countries, 173);
    assert_eq!(part_cases, 4);
    assert!(country_bounds().get("ru").is_none());
    assert_eq!(country_bounds()["RU"].min_lat, -18.);
}
