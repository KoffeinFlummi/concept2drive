// TODO
#![allow(dead_code)]

use std::convert::TryFrom;
use std::time::Duration;

use chrono;

use crate::error::*;

#[derive(Debug)]
pub enum WorkoutType {
    FreeRow = 0x01,
    SingleDistance = 0x03,
    SingleTime = 0x05,
    TimeInterval = 0x06,
    DistanceInterval = 0x07,
    VariableInterval = 0x08,
    SingleCalorie = 0x0A
}

impl TryFrom<u8> for WorkoutType {
    type Error = ParserError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(WorkoutType::FreeRow),
            0x03 => Ok(WorkoutType::SingleDistance),
            0x05 => Ok(WorkoutType::SingleTime),
            0x06 => Ok(WorkoutType::TimeInterval),
            0x07 => Ok(WorkoutType::DistanceInterval),
            0x08 => Ok(WorkoutType::VariableInterval),
            0x0A => Ok(WorkoutType::SingleCalorie),
            _ => Err(ParserError::default())
        }
    }
}

impl std::fmt::Display for WorkoutType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::FreeRow => "Free Row",
            Self::SingleDistance => "Distance",
            Self::SingleTime => "Time",
            Self::TimeInterval => "Time Interval",
            Self::DistanceInterval => "Distance Interval",
            Self::VariableInterval => "Variable Interval",
            Self::SingleCalorie => "Calories",
        })
    }
}

#[derive(Debug)]
pub struct Workout {
    pub workout_type: WorkoutType,
    pub serial_number: u32,
    pub datetime: chrono::NaiveDateTime,
    pub user_id: u16,   // TODO: needed?
    pub record_id: u16, // TODO: needed?
    pub total_distance: u32,
    pub total_work_duration: Duration,
    /// only set for intervals
    pub total_rest_duration: Option<Duration>,
    /// only set for single workouts
    pub spm: Option<u32>,
    /// splits for single workouts, intervals for interval workouts
    pub frames: Vec<WorkoutFrame>
}

impl Workout {
    pub fn watts(&self) -> f64 {
        let pace: f64 = self.total_work_duration.as_secs() as f64 / self.total_distance as f64;
        2.8 / pace.powi(3)
    }

    pub fn cal_hr(&self) -> f64 {
        (self.watts() * 3.44) + 300.0
    }

    pub fn cal_hr_weight_corrected(&self, weight: f64) -> f64 {
        (self.watts() * 3.44) + (1.714 * 2.2046 * weight)
    }

    pub fn pace(&self) -> Duration {
        let splits = std::cmp::max(self.total_distance / 500, 1);
        Duration::from_millis(self.total_work_duration.as_millis() as u64 / splits as u64)
    }

    pub fn heart_rate(&self) -> Option<u32> {
        if self.frames.len() == 0 {
            return None;
        }

        if self.frames.iter().any(|f| f.work_heart_rate.is_none()) {
            return None;
        }

        Some(self.frames.iter()
            .map(|f| f.work_heart_rate.unwrap())
            .sum::<u32>() / self.frames.len() as u32)
    }

    pub fn work_duration_string(&self) -> String {
        // TODO there has to be a better way
        duration_to_string(&self.total_work_duration)
    }

    pub fn rest_duration_string(&self) -> String {
        // TODO there has to be a better way
        self.total_rest_duration
            .map(|d| duration_to_string(&d))
            .unwrap_or_default()
    }

    pub fn pace_string(&self) -> String {
        duration_to_string(&self.pace())
    }
}

#[derive(Debug)]
pub struct WorkoutFrame {
    pub distance: u32,
    pub work_duration: Duration,
    pub rest_duration: Option<Duration>,
    pub spm: u32,
    pub work_heart_rate: Option<u32>,
    pub rest_heart_rate: Option<u32>,
}

impl WorkoutFrame {
    pub fn watts(&self) -> f64 {
        let pace: f64 = self.work_duration.as_secs() as f64 / self.distance as f64;
        2.8 / pace.powi(3)
    }

    pub fn cal_hr(&self) -> f64 {
        (self.watts() * 3.44) + 300.0
    }

    pub fn cal_hr_weight_corrected(&self, weight: f64) -> f64 {
        (self.watts() * 3.44) + (1.714 * 2.2046 * weight)
    }

    pub fn pace(&self) -> Duration {
        let splits = std::cmp::max(self.distance / 500, 1);
        Duration::from_millis(self.work_duration.as_millis() as u64 / splits as u64)
    }

    pub fn work_duration_string(&self) -> String {
        // TODO there has to be a better way
        duration_to_string(&self.work_duration)
    }

    pub fn rest_duration_string(&self) -> String {
        // TODO there has to be a better way
        self.rest_duration
            .map(|d| duration_to_string(&d))
            .unwrap_or_default()
    }

    pub fn pace_string(&self) -> String {
        duration_to_string(&self.pace())
    }
}

pub fn duration_to_string(duration: &Duration) -> String {
    if duration.as_secs() > 3600 {
        format!("{}:{:02}:{:02}.{}",
            duration.as_secs() / 3600,
            (duration.as_secs() / 60) % 60,
            duration.as_secs() % 60,
            duration.subsec_millis() / 100
        )
    } else {
        format!("{}:{:02}.{}",
            duration.as_secs() / 60,
            duration.as_secs() % 60,
            duration.subsec_millis() / 100
        )
    }
}
