concept2drive
=============

Library and command-line tool for initializing and reading Concept2 flash drives.

**This should be considered WIP. The initialization and the passive (read-only) functions work, but the firmware stuff has not been properly tested. Use at your own risk, maybe make a backup of your flash drive. Don't try to flash the PM5 firmware with this unless you are willing to risk bricking it. Also, some more exotic workout types (weird interval stuff) have not been thoroughly tested.**

## Setup

```
$ cargo install --path .
```

## Usage

Before initializing a flash drive, format the drive with one big partition. The filesystem is created by the tool, but `mkfs.fat` needs to be installed and available in `PATH`. The tool expects the path to the partition, not the drive. So if your stick is `/dev/sdd/`, pass `/dev/sdd1` to the tool.

To see command line options, see `concept2drive --help`.

## Making drive read/writeable by user

The application needs read (and for initializing, write) access to the block device of your USB drive. To avoid running the program as root, you can permanently grant your user access to the device with the following udev rule:

```
# /etc/udev/rules.d/99-concept2.rules

SUBSYSTEM=="block",ACTION=="add",ENV{ID_SERIAL}=="Intenso_Slim_Line_18121900016239-0:0",OWNER="flummi",GROUP="wheel",MODE="0664"
```

To find out the serial of your device, run `sudo udevadm monitor -p` as root before plugging in your device (Use the `SERIAL_ID` from the `block` subsystem, not the one from the `usb` subsystem. It's generally the last serial you see.). Replace the user name with yours.

Run `sudo udevadm control --reload-rules && sudo udevadm trigger` to reload and apply the rules without rebooting.

## References

- https://github.com/mbottini/concept2haskell/blob/master/writeup.md
- https://git.gutmet.org/pm5conv/dataformat
