//! Real JSON data phase. Labels/font metrics and QColor construction are stage5.
use crate::data::data_statistics_chart::*;
use serde_json::Value;
fn array(v: &Value) -> &[Value] {
    v.as_array().map_or(&[], Vec::as_slice)
}
fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
fn number(v: &Value) -> f64 {
    v.as_f64().unwrap_or(0.)
}
pub fn statistical_chart_from_json(json: &[u8]) -> StatisticalChart {
    let json = json.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(json);
    let Ok(root) = serde_json::from_slice::<Value>(json) else {
        return StatisticalChart::default();
    };
    if !root.is_object() {
        return StatisticalChart::default();
    }
    let columns = array(&root["columns"]);
    if columns.is_empty() {
        return StatisticalChart::default();
    }
    let hidden = array(&root["hidden"]).iter().map(text).collect::<Vec<_>>();
    let mut result = StatisticalChart::default();
    let tick = text(&root["yTickFormatter"]);
    result.currency = if tick.contains("TON") {
        StatisticalCurrency::Ton
    } else if tick.contains("XTR") {
        StatisticalCurrency::Credits
    } else {
        StatisticalCurrency::None
    };
    let mut id = 0;
    for column in columns {
        let a = array(column);
        if a.is_empty() {
            return StatisticalChart::default();
        }
        let key = text(&a[0]);
        if key == "x" {
            result.x.extend(a[1..].iter().map(number));
        } else {
            id += 1;
            let mut line = Line {
                id,
                id_string: key.into(),
                is_hidden_on_start: hidden.contains(&key),
                ..Default::default()
            };
            for v in &a[1..] {
                let v = number(v).round() as i64;
                line.y.push(v);
                line.max_value = line.max_value.max(v);
                line.min_value = line.min_value.min(v);
            }
            result.lines.push(line);
        }
        result.time_step = if result.x.len() > 1 {
            (result.x[1] - result.x[0]).max(1.)
        } else {
            86400000.
        };
        result.measure_data();
    }
    if result.x.is_empty() || result.lines.iter().any(|l| l.y.len() != result.x.len()) {
        return StatisticalChart::default();
    }
    if result.max_value == result.min_value {
        if result.min_value != 0 {
            result.min_value = 0;
        } else {
            result.max_value = 1;
        }
    }
    let sub = &root["subchart"];
    if let Some(b) = sub["show"].as_bool() {
        result.is_footer_hidden = !b;
    }
    let (mut min, mut max) = (0, result.x.len() - 1);
    let zoom = array(&sub["defaultZoom"]);
    if let Some((a, b)) = zoom.first().zip(zoom.last()) {
        for (i, &x) in result.x.iter().enumerate() {
            if x == number(a) {
                min = i;
            }
            if x == number(b) {
                max = i;
            }
        }
    }
    result.default_zoom_x_index = Zoom {
        min: min.min(max) as f64,
        max: min.max(max) as f64,
    };
    result.has_percentages = root["percentage"].as_bool().unwrap_or(false);
    result.week_format = text(&root["xTooltipFormatter"]).contains("'week'");
    for line in &mut result.lines {
        if let Some(spec) = root["colors"][&line.id_string].as_str() {
            let m = tg_base::qthelp_regex::regex_match(
                "(.*)(#.*)",
                spec,
                tg_base::qthelp_regex::RegExOptions::default(),
            );
            if m.has_match() {
                line.color_key = m.captured(1).into();
                line.color_spec = Some(m.captured(2).into());
            }
        }
        if let Some(name) = root["names"][&line.id_string].as_str() {
            line.name = name.replace('-', "—");
        }
    }
    result
}
