use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, TimeZone, Timelike, Weekday};

use crate::app::Checkpoint;

pub const UNIT: u32 = 15;

#[derive(Default)]
pub struct TimeSpan {
    pub units: u16,
}

pub struct Week {
    pub mon: Vec<Checkpoint>,
    pub tue: Vec<Checkpoint>,
    pub wed: Vec<Checkpoint>,
    pub thu: Vec<Checkpoint>,
    pub fri: Vec<Checkpoint>,
    pub unregistered_checkpoints: Vec<Checkpoint>,
    pub selected_weekday: Weekday,
    pub selected_checkpoint_idx: usize,
}

impl Week {
    pub fn new() -> Self {
        Self {
            mon: vec![],
            tue: vec![],
            wed: vec![],
            thu: vec![],
            fri: vec![],
            unregistered_checkpoints: vec![],
            selected_weekday: Weekday::Mon,
            selected_checkpoint_idx: 0,
        }
    }
    pub fn active_day(&self) -> &Vec<Checkpoint> {
        match self.selected_weekday {
            Weekday::Mon => &self.mon,
            Weekday::Tue => &self.tue,
            Weekday::Wed => &self.wed,
            Weekday::Thu => &self.thu,
            Weekday::Fri => &self.fri,
            Weekday::Sat => unimplemented!(),
            Weekday::Sun => unimplemented!(),
        }
    }

    pub fn select_next_checkpoint(&mut self) {
        if self.active_day().len() > self.selected_checkpoint_idx + 2 {
            self.selected_checkpoint_idx += 1;
        }
    }

    pub fn select_prev_checkpoint(&mut self) {
        self.selected_checkpoint_idx = if self.selected_checkpoint_idx > 0 {
            self.selected_checkpoint_idx - 1
        } else {
            0
        };
    }

    pub fn select_next_day(&mut self) {
        self.selected_weekday = match self.selected_weekday {
            Weekday::Mon => Weekday::Tue,
            Weekday::Tue => Weekday::Wed,
            Weekday::Wed => Weekday::Thu,
            Weekday::Thu => Weekday::Fri,
            Weekday::Fri => Weekday::Mon,
            Weekday::Sat => unimplemented!(),
            Weekday::Sun => unimplemented!(),
        };

        self.select_max_checkpoint_idx();
    }

    fn select_max_checkpoint_idx(&mut self) {
        self.selected_checkpoint_idx = match self.active_day().len() {
            0..1 => 0,
            active_day_len if self.selected_checkpoint_idx > active_day_len - 1 => {
                active_day_len - 2
            }
            _ => self.selected_checkpoint_idx,
        };
    }

    pub fn select_prev_day(&mut self) {
        self.selected_weekday = match self.selected_weekday {
            Weekday::Mon => Weekday::Fri,
            Weekday::Tue => Weekday::Mon,
            Weekday::Wed => Weekday::Tue,
            Weekday::Thu => Weekday::Wed,
            Weekday::Fri => Weekday::Thu,
            Weekday::Sat => unimplemented!(),
            Weekday::Sun => unimplemented!(),
        };

        self.select_max_checkpoint_idx();
    }

    pub fn append_checkpoint(&mut self, checkpoint: Checkpoint) {
        self.active_day_mut().push(checkpoint);
    }

    fn active_day_mut(&mut self) -> &mut Vec<Checkpoint> {
        match self.selected_weekday {
            Weekday::Mon => &mut self.mon,
            Weekday::Tue => &mut self.tue,
            Weekday::Wed => &mut self.wed,
            Weekday::Thu => &mut self.thu,
            Weekday::Fri => &mut self.fri,
            Weekday::Sat => unimplemented!(),
            Weekday::Sun => unimplemented!(),
        }
    }

    pub fn next_checkpoint(&self) -> Option<&Checkpoint> {
        let day = self.active_day();
        if day.len() > self.selected_checkpoint_idx + 1 {
            Some(&day[self.selected_checkpoint_idx + 1])
        } else {
            None
        }
    }

    pub fn next_checkpoint_mut(&mut self) -> Option<&mut Checkpoint> {
        let next_idx = self.selected_checkpoint_idx + 1;

        let day = self.active_day_mut();
        if day.len() > next_idx {
            Some(&mut day[next_idx])
        } else {
            None
        }
    }

    pub fn selected_checkpoint_mut(&mut self) -> Option<&mut Checkpoint> {
        match self.selected_weekday {
            Weekday::Mon => {
                if self.mon.len() > self.selected_checkpoint_idx {
                    Some(&mut self.mon[self.selected_checkpoint_idx])
                } else {
                    None
                }
            }
            Weekday::Tue => {
                if self.tue.len() > self.selected_checkpoint_idx {
                    Some(&mut self.tue[self.selected_checkpoint_idx])
                } else {
                    None
                }
            }
            Weekday::Wed => {
                if self.wed.len() > self.selected_checkpoint_idx {
                    Some(&mut self.wed[self.selected_checkpoint_idx])
                } else {
                    None
                }
            }
            Weekday::Thu => {
                if self.thu.len() > self.selected_checkpoint_idx {
                    Some(&mut self.thu[self.selected_checkpoint_idx])
                } else {
                    None
                }
            }
            Weekday::Fri => {
                if self.fri.len() > self.selected_checkpoint_idx {
                    Some(&mut self.fri[self.selected_checkpoint_idx])
                } else {
                    None
                }
            }
            Weekday::Sat => None,
            Weekday::Sun => None,
        }
    }

    pub fn selected_checkpoint(&self) -> Option<&Checkpoint> {
        match self.selected_weekday {
            Weekday::Mon => {
                if self.mon.len() > self.selected_checkpoint_idx {
                    Some(&self.mon[self.selected_checkpoint_idx])
                } else {
                    None
                }
            }
            Weekday::Tue => {
                if self.tue.len() > self.selected_checkpoint_idx {
                    Some(&self.tue[self.selected_checkpoint_idx])
                } else {
                    None
                }
            }
            Weekday::Wed => {
                if self.wed.len() > self.selected_checkpoint_idx {
                    Some(&self.wed[self.selected_checkpoint_idx])
                } else {
                    None
                }
            }
            Weekday::Thu => {
                if self.thu.len() > self.selected_checkpoint_idx {
                    Some(&self.thu[self.selected_checkpoint_idx])
                } else {
                    None
                }
            }
            Weekday::Fri => {
                if self.fri.len() > self.selected_checkpoint_idx {
                    Some(&self.fri[self.selected_checkpoint_idx])
                } else {
                    None
                }
            }
            Weekday::Sat => None,
            Weekday::Sun => None,
        }
    }
}

impl Default for Week {
    fn default() -> Self {
        Self::new()
    }
}

pub fn round_to_nearest_fifteen_minutes<Tz: TimeZone>(dt: DateTime<Tz>) -> DateTime<Tz> {
    let minute = dt.minute();
    let remainder = minute % 15;

    let rounded_dt = if remainder >= 8 {
        // Round up
        let minutes_to_add = 15 - remainder;
        dt + Duration::minutes(minutes_to_add as i64)
    } else {
        // Round down
        let minutes_to_subtract = remainder;
        dt - Duration::minutes(minutes_to_subtract as i64)
    };

    // Zero out seconds and microseconds
    rounded_dt
        .with_second(0)
        .unwrap()
        .with_nanosecond(0)
        .unwrap()
    /*
        // Get time components
        let minute = dt.minute();
        let second = dt.second();

        // Calculate total seconds and nanos into the current hour
        let total_secs = minute * 60 + second;

        // Duration of 15 minutes in seconds
        let fifteen_mins_secs = UNIT * 60;

        // Calculate the nearest 15-minute mark
        let rounded_secs =
            ((total_secs as f64 / fifteen_mins_secs as f64).round() * fifteen_mins_secs as f64) as i64;

        // Create a duration from the start of the hour
        let duration_from_hour_start = Duration::seconds(rounded_secs);

        // Start of the current hour
        let hour_start = dt.with_minute(0).unwrap().with_second(0).unwrap();

        // Add the rounded duration to the start of the hour
        hour_start + duration_from_hour_start
    */
}

/// Calculates the number of 15-minute intervals between two DateTime objects.
///
/// This function assumes that both DateTime objects are already rounded to 15-minute intervals.
/// If they are not, the result may not be accurate.
///
/// # Arguments
///
/// * `start` - The starting DateTime, assumed to be rounded to 15 minutes
/// * `end` - The ending DateTime, assumed to be rounded to 15 minutes
///
/// # Returns
///
/// The number of 15-minute intervals between the two DateTimes.
/// Returns a positive number if `end` is after `start`, or a negative number if `end` is before `start`.
pub fn count_fifteen_minute_intervals<Tz: TimeZone>(start: DateTime<Tz>, end: DateTime<Tz>) -> i64 {
    // Calculate the duration between the two DateTimes
    let duration = end.signed_duration_since(start);

    // Convert the duration to minutes
    let minutes = duration.num_minutes();

    // Divide by 15 to get the number of 15-minute intervals
    minutes / UNIT as i64
}

/// Converts minutes to human readable string
///
/// # Arguments
///
/// * `minutes` - The number of minutes to convert
///
/// # Returns
///
/// A human-readable string representation of the duration (e.g., "2h 30m", "45m", "1h")
pub fn human_duration(minutes: u32) -> String {
    if minutes == 0 {
        return "0m".to_string();
    }

    let hours = minutes / 60;
    let remaining_minutes = minutes % 60;

    match (hours, remaining_minutes) {
        (0, m) => format!("{}m", m),
        (h, 0) => format!("{}h", h),
        (h, m) => format!("{}h{}m", h, m),
    }
}

pub fn time_spans(checkpoints: &[Checkpoint]) -> Vec<TimeSpan> {
    // If we have fewer than 2 checkpoints, we can't calculate any time spans
    if checkpoints.len() < 2 {
        return Vec::new();
    }

    let mut spans = Vec::new();

    // Iterate through consecutive pairs of checkpoints
    for i in 0..checkpoints.len() - 1 {
        let start_time = checkpoints[i].time;
        let end_time = checkpoints[i + 1].time;

        // Round both times to the nearest 15 minutes
        let rounded_start = round_to_nearest_fifteen_minutes(start_time);
        let rounded_end = round_to_nearest_fifteen_minutes(end_time);

        // Calculate the number of 15-minute intervals
        let intervals = count_fifteen_minute_intervals(rounded_start, rounded_end);

        // Create a TimeSpan with the calculated number of intervals
        // Convert to u32 since we expect positive intervals between consecutive checkpoints
        let time_span = TimeSpan {
            units: intervals.max(0) as u16,
        };

        spans.push(time_span);
    }
    spans
}

pub fn current_date_minus_seven_days() -> DateTime<Local> {
    Local::now() - Duration::days(7)
}

/// Returns all Mondays in the given month as DateTime<Local> objects.
///
/// # Arguments
///
/// * `month` - The month (1-12) for which to find all Mondays
///
/// # Returns
///
/// A vector of DateTime<Local> objects representing all Mondays in the specified month.
/// Returns an empty vector if the month is invalid (not 1-12).
pub fn get_mondays_in_month(month: u32) -> Vec<NaiveDate> {
    if !(1..=12).contains(&month) {
        return Vec::new();
    }

    let now = Local::now().naive_local().date();
    let now_year = now.year();

    let mut mondays = Vec::new();

    // Get the first day of the month
    let first_day = match NaiveDate::from_ymd_opt(now_year, month, 1) {
        Some(date) => date,
        None => return Vec::new(),
    };

    // Find the first Monday of the month
    let days_until_monday = match first_day.weekday() {
        Weekday::Mon => 0,
        Weekday::Tue => -1,
        Weekday::Wed => -2,
        Weekday::Thu => -3,
        Weekday::Fri => -4,
        Weekday::Sat => -5,
        Weekday::Sun => -6,
    };

    let first_monday = first_day + Duration::days(days_until_monday);

    // Collect all Mondays in the month
    let mut current_monday = first_monday;
    while current_monday.month() <= month && current_monday <= now {
        // Convert to DateTime<Local> at midnight
        // if let Some(datetime) = Local
        //     .from_local_datetime(&current_monday.and_hms_opt(0, 0, 0).unwrap())
        //     .single()
        // {
        //     mondays.push(datetime);
        // }
        mondays.push(current_monday);
        current_monday += Duration::days(7);
    }

    mondays
}
