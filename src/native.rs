// TODO
#![allow(dead_code)]
#![allow(unused_variables)]

use std::convert::TryInto;
use std::io::Read;
use std::time::Duration;

use byteorder::{BigEndian, LittleEndian, ReadBytesExt};

use crate::error::*;
use crate::workouts::*;

#[derive(Debug, Default)]
pub struct LogDataAccessTableEntry {
    pub magic: u8,
    pub workout_type: u8,
    interval_rest_time: u16,
    workout_name: [u8; 2],
    unknown_1: [u8; 2],
    timestamp: u16,
    unknown_2: [u8; 2],
    num_splits: u16,
    duration_or_distance: u16,
    pub record_offset: u16,
    unknown_3: [u8; 6],
    pub record_size: u16,
    index: u16,
    unknown_4: [u8; 4]
}

impl LogDataAccessTableEntry {
    pub fn read<R: Read>(f: &mut R) -> Result<Self,ParserError> {
        let magic = f.read_u8()?;
        let workout_type = f.read_u8()?;
        let interval_rest_time = f.read_u16::<LittleEndian>()?;
        let mut workout_name = [0; 2];
        f.read_exact(&mut workout_name)?;
        let mut unknown_1 = [0; 2];
        f.read_exact(&mut unknown_1)?;
        let timestamp = f.read_u16::<BigEndian>()?;
        let mut unknown_2 = [0; 2];
        f.read_exact(&mut unknown_2)?;
        let num_splits = f.read_u16::<LittleEndian>()?;
        let duration_or_distance = f.read_u16::<LittleEndian>()?;
        let record_offset = f.read_u16::<LittleEndian>()?;
        let mut unknown_3 = [0; 6];
        f.read_exact(&mut unknown_3)?;
        let record_size = f.read_u16::<LittleEndian>()?;
        let index = f.read_u16::<LittleEndian>()?;
        let mut unknown_4 = [0; 4];
        f.read_exact(&mut unknown_4)?;

        if magic != 0xf0 && magic != 0xff && magic != 0x70 {
            return Err(ParserError::default());
        }

        Ok(Self {
            magic,
            workout_type,
            interval_rest_time,
            workout_name,
            unknown_1,
            timestamp,
            unknown_2,
            num_splits,
            duration_or_distance,
            record_offset,
            unknown_3,
            record_size,
            index,
            unknown_4
        })
    }
}

#[derive(Debug)]
pub enum LogDataStorageEntry {
    Single(SingleEntry),
    FixedInterval(FixedIntervalEntry),
    VariableInterval(VariableIntervalEntry),
}

impl LogDataStorageEntry {
    pub fn read<R: Read>(f: &mut R) -> Result<Self,ParserError> {
        let magic = f.read_u8()?;
        let workout_type = f.read_u8()?;

        match workout_type {
            0x01 | 0x03 | 0x05 | 0x0A => {
                Ok(Self::Single(SingleEntry::read(f, magic, workout_type.try_into()?)?))
            },
            0x06 | 0x07 => {
                Ok(Self::FixedInterval(FixedIntervalEntry::read(f, magic, workout_type.try_into()?)?))
            },
            0x08 => {
                Ok(Self::VariableInterval(VariableIntervalEntry::read(f, magic, workout_type.try_into()?)?))
            },
            _ => {
                Err(ParserError::default())
            }
        }
    }
}

impl Into<Workout> for LogDataStorageEntry {
    fn into(self) -> Workout {
        match self {
            Self::Single(entry) => entry.into(),
            Self::FixedInterval(entry) => entry.into(),
            Self::VariableInterval(entry) => entry.into()
        }
    }
}

#[derive(Debug)]
pub struct SingleEntry {
    magic: u8,
    workout_type: WorkoutType,
    unknown_1: [u8; 2],
    serial_number: u32,
    timestamp: u32,
    user_id: u16,
    unknown_2: [u8; 4],
    record_id: u8,
    magic_2: [u8; 3],
    total_duration: u16,
    total_distance: u32,
    spm: u8,
    split_info: u8,
    split_size: u16,
    unknown_3: [u8; 18],
    frames: Vec<SingleFrame>
}

impl SingleEntry {
    pub fn read<R: Read>(f: &mut R, magic: u8, workout_type: WorkoutType) -> Result<Self,std::io::Error> {
        let mut unknown_1 = [0; 2];
        f.read_exact(&mut unknown_1)?;
        let serial_number = f.read_u32::<BigEndian>()?;
        let timestamp = f.read_u32::<BigEndian>()?;
        let user_id = f.read_u16::<BigEndian>()?;
        let mut unknown_2 = [0; 4];
        f.read_exact(&mut unknown_2)?;
        let record_id = f.read_u8()?;
        let mut magic_2 = [0; 3];
        f.read_exact(&mut magic_2)?;
        let total_duration = f.read_u16::<BigEndian>()?;
        let total_distance = f.read_u32::<BigEndian>()?;
        let spm = f.read_u8()?;
        let split_info = f.read_u8()?;
        let split_size = f.read_u16::<BigEndian>()?;
        let mut unknown_3 = [0; 18];
        f.read_exact(&mut unknown_3)?;

        let num_frames: u32 = match workout_type {
            WorkoutType::FreeRow | WorkoutType::SingleDistance => {
                total_distance / (split_size as u32) +
                    if total_distance % (split_size as u32) > 0 { 1 } else { 0 }
            },
            WorkoutType::SingleTime => {
                todo!()
            },
            WorkoutType::SingleCalorie => {
                todo!()
            },
            _ => { unreachable!() }
        };

        let mut frames = Vec::with_capacity(num_frames as usize);

        for _i in 0..num_frames {
            frames.push(SingleFrame::read(f)?);
        }

        Ok(Self {
            magic,
            workout_type,
            unknown_1,
            serial_number,
            timestamp,
            user_id,
            unknown_2,
            record_id,
            magic_2,
            total_duration,
            total_distance,
            spm,
            split_info,
            split_size,
            unknown_3,
            frames
        })
    }
}

impl Into<Workout> for SingleEntry {
    fn into(self) -> Workout {
        let mut frames: Vec<WorkoutFrame> = self.frames.into_iter().map(|f| f.into()).collect();

        for f in frames.iter_mut() {
            match self.workout_type {
                WorkoutType::FreeRow | WorkoutType::SingleDistance => {
                    f.distance = self.split_size as u32;
                },
                WorkoutType::SingleTime => {
                    todo!()
                },
                WorkoutType::SingleCalorie => {
                    todo!()
                },
                _ => { unreachable!() }
            }
        }

        Workout {
            workout_type: self.workout_type,
            serial_number: self.serial_number,
            datetime: decode_timestamp(self.timestamp),
            user_id: self.user_id,
            record_id: self.record_id as u16,
            total_distance: self.total_distance,
            total_work_duration: Duration::from_millis(self.total_duration as u64 * 100),
            total_rest_duration: None,
            spm: Some(self.spm.into()),
            frames
        }
    }
}

#[derive(Debug)]
pub struct FixedIntervalEntry {
    magic: u8,
    workout_type: WorkoutType,
    unknown_1: [u8; 2],
    serial_number: u32,
    timestamp: u32,
    user_id: u16,
    unknown_2: [u8; 4],
    record_id: u8,
    num_splits: u8,
    split_size: u16,
    interval_rest_time: u16,
    total_work_duration: u32,
    total_rest_distance: u16,
    unknown_3: [u8; 22],
    frames: Vec<FixedIntervalFrame>
}

impl FixedIntervalEntry {
    pub fn read<R: Read>(f: &mut R, magic: u8, workout_type: WorkoutType) -> Result<Self,std::io::Error> {
        todo!();
    }
}

impl Into<Workout> for FixedIntervalEntry {
    fn into(self) -> Workout {
        todo!();
    }
}

#[derive(Debug)]
pub struct VariableIntervalEntry {
    magic: u8,
    workout_type: WorkoutType,
    unknown_1: [u8; 2],
    serial_number: u32,
    timestamp: u32,
    user_id: u16,
    unknown_2: [u8; 4],
    record_id: u8,
    num_splits: u16,
    total_work_duration: u32,
    total_work_distance: u32,
    unknown_3: [u8; 24],
    frames: Vec<VariableIntervalFrame>
}

impl VariableIntervalEntry {
    pub fn read<R: Read>(f: &mut R, magic: u8, workout_type: WorkoutType) -> Result<Self,std::io::Error> {
        todo!();
    }
}

impl Into<Workout> for VariableIntervalEntry {
    fn into(self) -> Workout {
        todo!();
    }
}

#[derive(Debug)]
pub struct SingleFrame {
    duration_or_distance: u16,
    heart_rate: u8,
    spm: u8,
    unknown: [u8; 28]
}

impl SingleFrame {
    pub fn read<R: Read>(f: &mut R) -> Result<Self,std::io::Error> {
        let duration_or_distance = f.read_u16::<BigEndian>()?;
        let heart_rate = f.read_u8()?;
        let spm = f.read_u8()?;
        let mut unknown = [0; 28];
        f.read_exact(&mut unknown)?;

        // TODO: read heart min, max, median/mean

        Ok(Self {
            duration_or_distance,
            heart_rate,
            spm,
            unknown
        })
    }
}

impl Into<WorkoutFrame> for SingleFrame {
    fn into(self) -> WorkoutFrame {
        // Depending on the type, either distance or duration
        // will have to be overwritten
        WorkoutFrame {
            distance: self.duration_or_distance as u32,
            work_duration: Duration::from_millis(self.duration_or_distance as u64 * 100),
            rest_duration: None,
            spm: self.spm as u32,
            work_heart_rate: if self.heart_rate > 0 { Some(self.heart_rate as u32) } else { None },
            rest_heart_rate: None,
        }
    }
}

#[derive(Debug)]
pub struct FixedIntervalFrame {
}

#[derive(Debug)]
pub struct VariableIntervalFrame {
}


pub fn decode_timestamp(timestamp: u32) -> chrono::NaiveDateTime {
    let year = 2000 + ((timestamp & (0b1111111 << 25)) >> 25);
    let day = (timestamp & (0b11111 << 20)) >> 20;
    let month = (timestamp & (0b1111 << 16)) >> 16;

    let hour = (timestamp & (0b11111111 << 8)) >> 8;
    let minute = timestamp & 0b1111111;

    let date = chrono::NaiveDate::from_ymd(year as i32, month, day);
    let time = chrono::NaiveTime::from_hms_milli(hour, minute, 0, 0);
    date.and_time(time)
}
