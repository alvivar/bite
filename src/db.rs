use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::sleep,
    time::Duration,
};

pub const DB_PATH: &str = "./data";
const DB_NAME: &str = "db.bin";
const DB_TEMP_NAME: &str = "db.bin.tmp";

pub struct DB {
    data: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    dir: PathBuf,
    pub modified: Arc<AtomicBool>,
}

impl DB {
    pub fn new(data: Arc<Mutex<BTreeMap<String, Vec<u8>>>>, dir: &Path) -> DB {
        let modified = Arc::new(AtomicBool::new(false));

        DB {
            data,
            dir: dir.to_path_buf(),
            modified,
        }
    }

    /// Saves the data every `throttle` seconds when modified. Only returns
    /// when saving fails.
    pub fn handle(&mut self, throttle: u64) -> io::Result<()> {
        loop {
            sleep(Duration::new(throttle, 0));

            if self.modified.swap(false, Ordering::Relaxed) {
                self.save_to_file()?;
            }
        }
    }

    /// Loads the saved data. A missing or empty file means there is nothing
    /// saved yet, anything that can't be read or deserialized is an error and
    /// leaves the data untouched.
    pub fn load_from_file(&self) -> io::Result<()> {
        fs::create_dir_all(&self.dir)
            .map_err(|err| with_path(err, "could not create", &self.dir))?;

        let file = self.dir.join(DB_NAME);
        let content = match fs::read(&file) {
            Ok(content) => content,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(with_path(err, "could not read", &file)),
        };

        if content.is_empty() {
            return Ok(());
        }

        let data = bincode::deserialize::<BTreeMap<String, Vec<u8>>>(&content).map_err(|err| {
            let err = io::Error::new(ErrorKind::InvalidData, err);
            with_path(err, "could not deserialize", &file)
        })?;
        *self.data.lock().unwrap() = data;

        Ok(())
    }

    /// Writes a complete snapshot to a temporary file and then replaces the
    /// saved file with it, so an interrupted save never leaves a partial file.
    pub fn save_to_file(&self) -> io::Result<()> {
        let data = bincode::serialize(&*self.data.lock().unwrap()).map_err(io::Error::other)?;

        let temp = self.dir.join(DB_TEMP_NAME);
        write_synced(&temp, &data).map_err(|err| with_path(err, "could not write", &temp))?;

        let file = self.dir.join(DB_NAME);
        fs::rename(&temp, &file).map_err(|err| with_path(err, "could not replace", &file))?;

        info!("{DB_NAME} saved");

        Ok(())
    }
}

/// Writes the data and waits until the OS flushed it to disk.
fn write_synced(path: &Path, data: &[u8]) -> io::Result<()> {
    let mut file = File::create(path)?;
    file.write_all(data)?;
    file.sync_all()
}

fn with_path(err: io::Error, action: &str, path: &Path) -> io::Error {
    io::Error::new(err.kind(), format!("{action} {}: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, process};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!("bite-db-{}-{name}", process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn db(dir: &Path, entries: &[(&str, &[u8])]) -> DB {
        let map = entries
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_vec()))
            .collect();

        DB::new(Arc::new(Mutex::new(map)), dir)
    }

    fn map(db: &DB) -> BTreeMap<String, Vec<u8>> {
        db.data.lock().unwrap().clone()
    }

    #[test]
    fn save_replaces_existing_snapshot() {
        let dir = temp_dir("replace");
        db(&dir, &[("old", b"1")]).save_to_file().unwrap();

        let saved = db(&dir, &[("new", b"2")]);
        saved.save_to_file().unwrap();

        let loaded = db(&dir, &[]);
        loaded.load_from_file().unwrap();
        assert_eq!(map(&loaded), map(&saved));
        assert!(!dir.join(DB_TEMP_NAME).exists());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_or_empty_file_loads_nothing() {
        let dir = temp_dir("missing").join("data");
        let db = db(&dir, &[("key", b"value")]);

        db.load_from_file().unwrap();
        assert!(dir.is_dir());

        File::create(dir.join(DB_NAME)).unwrap();
        db.load_from_file().unwrap();

        assert_eq!(
            map(&db),
            BTreeMap::from([("key".into(), b"value".to_vec())])
        );

        fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn corrupt_file_fails_and_keeps_data() {
        let dir = temp_dir("corrupt");
        let snapshot =
            bincode::serialize(&BTreeMap::from([("key".to_string(), b"value".to_vec())])).unwrap();
        let truncated = &snapshot[..snapshot.len() - 1];
        fs::write(dir.join(DB_NAME), truncated).unwrap();

        let db = db(&dir, &[("current", b"data")]);
        let err = db.load_from_file().unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidData);
        assert_eq!(
            map(&db),
            BTreeMap::from([("current".into(), b"data".to_vec())])
        );
        assert_eq!(fs::read(dir.join(DB_NAME)).unwrap(), truncated);

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_save_keeps_previous_snapshot() {
        let dir = temp_dir("failed-save");
        db(&dir, &[("old", b"1")]).save_to_file().unwrap();
        let previous = fs::read(dir.join(DB_NAME)).unwrap();

        // A directory where the temporary file goes makes creating it fail.
        fs::create_dir(dir.join(DB_TEMP_NAME)).unwrap();
        assert!(db(&dir, &[("new", b"2")]).save_to_file().is_err());

        assert_eq!(fs::read(dir.join(DB_NAME)).unwrap(), previous);

        fs::remove_dir_all(dir).unwrap();
    }
}
