//! Helper for JSON serialization.

use std::{
    fs::File,
    io::{self, BufWriter},
    path::Path,
};

use serde::Serialize;
use serde_json::ser::{CompactFormatter, Formatter};

/// Convenience wrapper over an instance of `serde_json::ser::Formatter`.
pub struct Serializer {
    writer: BufWriter<File>,
    fmt: CompactFormatter,
    first_stack: Vec<bool>,
}

impl Serializer {
    /// Creates a new `Serializer`.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, io::Error> {
        Ok(Self {
            writer: BufWriter::new(File::create(path)?),
            fmt: CompactFormatter,
            first_stack: vec![],
        })
    }

    /// Begins an object.
    pub fn begin_object(&mut self) -> Result<(), io::Error> {
        self.fmt.begin_object(&mut self.writer)?;
        self.first_stack.push(true);
        Ok(())
    }

    /// Ends an object.
    pub fn end_object(&mut self) -> Result<(), io::Error> {
        self.fmt.end_object(&mut self.writer)?;
        self.first_stack.pop();
        Ok(())
    }

    /// Begins an object value.
    pub fn begin_object_value(&mut self) -> Result<(), io::Error> {
        self.fmt.begin_object_value(&mut self.writer)
    }

    /// Ends an object value.
    pub fn end_object_value(&mut self) -> Result<(), io::Error> {
        self.fmt.end_object_value(&mut self.writer)
    }

    /// Begins an array.
    pub fn begin_array(&mut self) -> Result<(), io::Error> {
        self.fmt.begin_array(&mut self.writer)?;
        self.first_stack.push(true);
        Ok(())
    }

    /// Ends an array.
    pub fn end_array(&mut self) -> Result<(), io::Error> {
        self.fmt.end_array(&mut self.writer)?;
        self.first_stack.pop();
        Ok(())
    }

    /// Begins an array value.
    pub fn begin_array_value(&mut self) -> Result<(), io::Error> {
        self.fmt
            .begin_array_value(&mut self.writer, *self.first_stack.last().unwrap())?;
        *self.first_stack.last_mut().unwrap() = false;
        Ok(())
    }

    /// Ends an array value.
    pub fn end_array_value(&mut self) -> Result<(), io::Error> {
        self.fmt.end_array_value(&mut self.writer)
    }

    /// Writes a serializable value.
    pub fn write<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), io::Error> {
        let mut ser = serde_json::Serializer::with_formatter(&mut self.writer, self.fmt.clone());
        value.serialize(&mut ser)?;
        Ok(())
    }

    /// Writes an object key.
    pub fn key(&mut self, key: &str) -> Result<(), io::Error> {
        self.fmt
            .begin_object_key(&mut self.writer, *self.first_stack.last().unwrap())?;
        self.write(key)?;
        self.fmt.end_object_key(&mut self.writer)?;
        *self.first_stack.last_mut().unwrap() = false;
        Ok(())
    }

    /// Writes an object key and value.
    pub fn entry<T: ?Sized + Serialize>(&mut self, key: &str, value: &T) -> Result<(), io::Error> {
        self.key(key)?;
        self.begin_object_value()?;
        self.write(value)?;
        self.end_object_value()?;
        Ok(())
    }
}
