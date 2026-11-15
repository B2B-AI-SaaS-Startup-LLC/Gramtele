//! Statistical model; label localization, QColor and font metrics remain stage5.
use crate::statistics::segment_tree::SegmentTree;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatisticalCurrency {
    #[default]
    None,
    Ton,
    Credits,
}
#[derive(Clone, Debug)]
pub struct Line {
    pub y: Vec<i64>,
    pub segment_tree: SegmentTree,
    pub id: i32,
    pub id_string: String,
    pub name: String,
    pub max_value: i64,
    pub min_value: i64,
    pub color_key: String,
    pub color_spec: Option<String>,
    pub is_hidden_on_start: bool,
}
impl Default for Line {
    fn default() -> Self {
        Self {
            y: vec![],
            segment_tree: Default::default(),
            id: 0,
            id_string: String::new(),
            name: String::new(),
            max_value: 0,
            min_value: i64::MAX,
            color_key: String::new(),
            color_spec: None,
            is_hidden_on_start: false,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Zoom {
    pub min: f64,
    pub max: f64,
}
#[derive(Clone, Debug)]
pub struct StatisticalChart {
    pub x: Vec<f64>,
    pub x_percentage: Vec<f64>,
    pub days_lookup: Vec<String>,
    pub lines: Vec<Line>,
    pub default_zoom_x_index: Zoom,
    pub max_value: i64,
    pub min_value: i64,
    pub one_day_percentage: f64,
    pub time_step: f64,
    pub is_footer_hidden: bool,
    pub has_percentages: bool,
    pub week_format: bool,
    pub currency: StatisticalCurrency,
    pub currency_rate: f64,
    pub day_string_max_width: i32,
}
impl Default for StatisticalChart {
    fn default() -> Self {
        Self {
            x: vec![],
            x_percentage: vec![],
            days_lookup: vec![],
            lines: vec![],
            default_zoom_x_index: Zoom::default(),
            max_value: 0,
            min_value: i64::MAX,
            one_day_percentage: 0.,
            time_step: 1.,
            is_footer_hidden: false,
            has_percentages: false,
            week_format: false,
            currency: Default::default(),
            currency_rate: 0.,
            day_string_max_width: 0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DayLabelsRequest {
    pub start: f64,
    pub time_step: f64,
    pub count: i32,
}
impl StatisticalChart {
    pub fn empty(&self) -> bool {
        self.lines.is_empty()
    }
    /// Source measure() numeric mutation phase; the returned request is rendered by5.
    pub fn measure_data(&mut self) -> Option<DayLabelsRequest> {
        let (&start, &end) = self.x.first().zip(self.x.last())?;
        let n = self.x.len();
        self.x_percentage = self
            .x
            .iter()
            .map(|x| {
                if n == 1 {
                    1.
                } else {
                    (*x - start) / (end - start)
                }
            })
            .collect();
        for line in &mut self.lines {
            self.max_value = self.max_value.max(line.max_value);
            self.min_value = self.min_value.min(line.min_value);
            line.segment_tree = SegmentTree::new(line.y.clone());
        }
        self.days_lookup.clear();
        self.one_day_percentage = self.time_step / (end - start);
        Some(DayLabelsRequest {
            start,
            time_step: self.time_step,
            count: ((end - start) / self.time_step) as i32 + 10,
        })
    }
    pub fn get_day_string(&self, i: usize) -> &str {
        &self.days_lookup[((self.x[i] - self.x[0]) / self.time_step) as usize]
    }
    pub fn find_start_index(&self, v: f64) -> i32 {
        if v == 0. || self.x_percentage.len() < 2 {
            return 0;
        }
        let (mut left, mut right) = (0, self.x_percentage.len() as i32 - 1);
        while left <= right {
            let m = (left + right) >> 1;
            let x = self.x_percentage[m as usize];
            if v < x && (m == 0 || v > self.x_percentage[m as usize - 1]) || v == x {
                return m;
            }
            if v < x {
                right = m - 1;
            } else if v > x {
                left = m + 1;
            }
        }
        left
    }
    pub fn find_end_index(&self, mut left: i32, v: f64) -> i32 {
        let was = left;
        let n = self.x_percentage.len() as i32;
        if v == 1. {
            return n - 1;
        }
        let mut right = n - 1;
        while left <= right {
            let m = (left + right) >> 1;
            let x = self.x_percentage[m as usize];
            if v > x && (m == n - 1 || v < self.x_percentage[m as usize + 1]) || v == x {
                return m;
            }
            if v < x {
                right = m - 1;
            } else if v > x {
                left = m + 1;
            }
        }
        was.max(right)
    }
    pub fn find_index(&self, mut left: i32, mut right: i32, v: f64) -> i32 {
        if v <= self.x_percentage[left as usize] {
            return left;
        }
        if v >= self.x_percentage[right as usize] {
            return right;
        }
        let n = self.x_percentage.len() as i32;
        while left <= right {
            let m = (left + right) >> 1;
            let x = self.x_percentage[m as usize];
            if v > x && (m == n - 1 || v < self.x_percentage[m as usize + 1]) || v == x {
                return m;
            }
            if v < x {
                right = m - 1;
            } else if v > x {
                left = m + 1;
            }
        }
        right
    }
}
#[derive(Clone, Debug, Default)]
pub struct StatisticalGraph {
    pub chart: StatisticalChart,
    pub zoom_token: String,
    pub error: String,
}
