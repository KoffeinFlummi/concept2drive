use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::fs::File;
use std::path::Path;

use byteorder::{BigEndian, ReadBytesExt};
use fatfs;
use fscommon;

use crate::error::*;
use crate::native::*;
use crate::workouts::*;

pub struct Drive {
    fs: fatfs::FileSystem<fscommon::BufStream<std::fs::File>>
}


impl Drive {
    pub fn new<P: AsRef<Path>>(drive_path: P, allow_writing: bool) -> Result<Self,std::io::Error> {
        let img_file = std::fs::OpenOptions::new()
            .read(true)
            .write(allow_writing)
            .open(drive_path)?;
        let buf_stream = fscommon::BufStream::new(img_file);
        let fs = fatfs::FileSystem::new(buf_stream, fatfs::FsOptions::new())?;

        Ok(Drive { fs })
    }

    pub fn init<P: AsRef<Path>>(drive_path: P, user_name: String) -> Result<Self,std::io::Error> {
        let mut name = user_name.into_bytes();
        if name.len() < 1 || name.len() > 6 {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Name needs to be <= 6 characters!"));
        }

        name.resize(6, 0x00);

        let status = std::process::Command::new("mkfs.fat")
            .arg(drive_path.as_ref())
            .stdout(std::process::Stdio::null())
            .status()?;
        if !status.success() {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to format drive."));
        }

        let img_file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(drive_path)?;
        let buf_stream = fscommon::BufStream::new(img_file);
        let fs = fatfs::FileSystem::new(buf_stream, fatfs::FsOptions::new())?;

        {
            let root_dir = fs.root_dir();

            root_dir.create_dir("Concept2")?;
            root_dir.create_dir("Concept2/DiagLog")?;
            root_dir.create_dir("Concept2/Firmware")?;
            let log_book = root_dir.create_dir("Concept2/Logbook")?;
            root_dir.create_dir("Concept2/Special")?;

            let mut file = log_book.create_file("DeviceLogInfo.bin")?;
            file.write(include_bytes!("data/DeviceLogInfo.bin"))?;
            let mut file = log_book.create_file("Favorites.bin")?;
            file.write(include_bytes!("data/Favorites.bin"))?;
            let mut file = log_book.create_file("LogDataAccessTbl.bin")?;
            file.write(include_bytes!("data/LogDataAccessTbl.bin"))?;
            let mut file = log_book.create_file("LogDataStorage.bin")?;
            file.write(include_bytes!("data/LogDataStorage.bin"))?;
            let mut file = log_book.create_file("LogStrokeInfo.bin")?;
            file.write(include_bytes!("data/LogStrokeInfo.bin"))?;
            let mut file = log_book.create_file("StrokeDataAccessTbl.bin")?;
            file.write(include_bytes!("data/StrokeDataAccessTbl.bin"))?;
            let mut file = log_book.create_file("StrokeDataStorage.bin")?;
            file.write(include_bytes!("data/StrokeDataStorage.bin"))?;
            let mut file = log_book.create_file("UserDynamic.bin")?;
            file.write(include_bytes!("data/UserDynamic.bin"))?;

            let mut file = log_book.create_file("UserStatic.bin")?;
            file.write(&[0x91, 0x00])?;
            file.write(&name)?;
            file.write(&[
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0xaf, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00])?;
        }

        //let mut file = log_book.create_file("UserStatic.bin")?;
        //UserStatic.bin

        //00000000: 9100 666c 756d 6d69 0000 0000 0000 0000  ..flummi........
        //00000010: 0000 0000 0000 0000 0000 0000 0000 0100  ................
        //00000020: 0000 0000 0000 0000 0000 00af 0000 0000  ................
        //00000030: 0000 0000 0000 0000 0000                 ..........

        Ok(Drive { fs })
    }

    /// Returns a tuple of the user id and user name that is configured
    /// on the drive.
    pub fn user(&mut self) -> Result<(u16,String),std::io::Error> {
        let mut user_static_file = self.fs.root_dir().open_file("Concept2/Logbook/UserStatic.bin")?;
        let mut buffer = [0; 6];

        user_static_file.seek(SeekFrom::Start(0x02))?;
        user_static_file.read_exact(&mut buffer)?;

        user_static_file.seek(SeekFrom::Start(0x2a))?;
        let user_id = user_static_file.read_u16::<BigEndian>()?;

        Ok((user_id, String::from_utf8(buffer.to_vec()).unwrap()))
    }

    pub fn workouts(&mut self) -> Result<Vec<Workout>,ParserError> {
        let mut access_table_file = self.fs.root_dir().open_file("Concept2/Logbook/LogDataAccessTbl.bin")?;
        let mut storage_file = self.fs.root_dir().open_file("Concept2/Logbook/LogDataStorage.bin")?;
        let mut access_table_entries: Vec<LogDataAccessTableEntry> = Vec::new();

        loop {
            let entry = LogDataAccessTableEntry::read(&mut access_table_file)?;

            // 0x70 was only encountered at the end
            if entry.magic == 0xff || entry.magic == 0x70 {
                break;
            }

            access_table_entries.push(entry);
        }

        let mut workouts = Vec::with_capacity(access_table_entries.len());

        for at_entry in access_table_entries {
            storage_file.seek(SeekFrom::Start(at_entry.record_offset.into()))?;

            let entry = LogDataStorageEntry::read(&mut storage_file)?;
            let workout = entry.into();
            workouts.push(workout);
        }

        Ok(workouts)
    }

    pub fn export_workouts<P: AsRef<Path>>(&mut self, _csv_path: P) -> Result<(),std::io::Error> {
        todo!();
    }

    // TODO: clear, write workouts

    pub fn firmwares(&mut self) -> Result<Vec<String>,std::io::Error> {
        let firmware_dir = self.fs.root_dir().open_dir("Concept2/Firmware");
        if let Err(_) = firmware_dir {
            return Ok(Vec::new());
        }

        let mut firmwares = Vec::new();

        for fw in firmware_dir.unwrap().iter() {
            let name = fw?.file_name();

            if name.chars().nth(0).unwrap() == '.' {
                continue;
            }

            if &name[(name.len()-3)..name.len()] != ".7z" {
                continue;
            }

            firmwares.push(name);
        }

        Ok(firmwares)
    }

    pub fn clear_firmwares(&mut self) -> Result<(),std::io::Error> {
        if let Err(_) = self.fs.root_dir().open_dir("Concept2/Firmware") {
            self.fs.root_dir().create_dir("Concept2/Firmware")?;
        }

        let firmware_dir = self.fs.root_dir().open_dir("Concept2/Firmware")?;

        for fw in firmware_dir.iter() {
            let name = fw?.file_name();

            if name.chars().nth(0) == Some('.') {
                continue;
            }

            firmware_dir.remove(&name)?;
        }

        Ok(())
    }

    pub fn write_firmware_callback<P: AsRef<Path>, F: Fn(u64,u64) -> ()>(
        &mut self,
        archive: P,
        progress_callback: F
    ) -> Result<(), std::io::Error> {
        let firmware_dir = self.fs.root_dir().open_dir("Concept2/Firmware")?;
        let archive_size: u64 = archive.as_ref().metadata()?.len();

        let output = std::process::Command::new("7z")
            .arg("l").arg("-ba").arg(archive.as_ref())
            .output()?.stdout;
        let output = String::from_utf8(output).unwrap();

        let mut files: HashMap<String,u64> = HashMap::new();
        let regex = regex::Regex::new(r"[^\s]+\.bin").unwrap();

        for line in output.split("\n") {
            if line.len() == 0 { break; }

            let size = line.split_whitespace().nth(3).unwrap().parse().unwrap();
            let name = regex.find(line).unwrap().as_str().to_string();
            files.insert(name, size);
        }

        let mut written: u64 = 0;
        let total_size: u64 = archive_size + files.values().sum::<u64>();

        for (name, size) in &files {
            progress_callback(written, total_size);
            let extracted = std::process::Command::new("7z")
                .arg("x").arg("-so").arg(archive.as_ref()).arg(name)
                .output()?.stdout;
            let mut cursor = Cursor::new(extracted);
            let mut target = firmware_dir.create_file(name)?;
            target.truncate()?;
            std::io::copy(&mut cursor, &mut target)?;
            written += size;
        }

        let archive_name = archive.as_ref().file_name().unwrap();
        let mut f = File::open(archive.as_ref())?;
        let mut target = firmware_dir.create_file(archive_name.to_str().unwrap())?;
        target.truncate()?;
        std::io::copy(&mut f, &mut target)?;
        written += archive_size;

        progress_callback(written, total_size);

        Ok(())
    }

    pub fn write_firmware<P: AsRef<Path>>(&mut self, archive: P) -> Result<(),std::io::Error> {
        self.write_firmware_callback(archive, |_,_| {})
    }
}
