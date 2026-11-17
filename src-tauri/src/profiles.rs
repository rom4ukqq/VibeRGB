use std::{
    fs::{self, File},
    io::ErrorKind,
    path::PathBuf,
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

use crate::lighting::Assignment;

#[derive(Clone, Deserialize, Serialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub assignments: Vec<Assignment>,
}

#[derive(Default, Deserialize, Serialize)]
struct FileContents {
    next_id: u64,
    profiles: Vec<Profile>,
}

pub struct Profiles(Mutex<PathBuf>);

impl Profiles {
    pub fn new(path: PathBuf) -> Self {
        Self(Mutex::new(path))
    }

    pub fn list(&self) -> Result<Vec<Profile>, String> {
        let path = self.0.lock().map_err(|error| error.to_string())?;
        Ok(read(&path)?.profiles)
    }

    pub fn save(&self, name: String, assignments: Vec<Assignment>) -> Result<Vec<Profile>, String> {
        if assignments.is_empty() {
            return Err("Apply lighting to at least one zone first.".into());
        }
        let name = normalized_name(&name)?;
        self.edit(move |file| {
            file.next_id = file
                .next_id
                .checked_add(1)
                .ok_or("The profile limit has been reached.")?;
            file.profiles.push(Profile {
                id: file.next_id.to_string(),
                name,
                assignments,
            });
            Ok(())
        })
    }

    pub fn rename(&self, id: &str, name: String) -> Result<Vec<Profile>, String> {
        let name = normalized_name(&name)?;
        self.edit(move |file| {
            let profile = file
                .profiles
                .iter_mut()
                .find(|profile| profile.id == id)
                .ok_or("Profile not found.")?;
            profile.name = name;
            Ok(())
        })
    }

    pub fn delete(&self, id: &str) -> Result<Vec<Profile>, String> {
        self.edit(|file| {
            let index = file
                .profiles
                .iter()
                .position(|profile| profile.id == id)
                .ok_or("Profile not found.")?;
            file.profiles.remove(index);
            Ok(())
        })
    }

    fn edit(
        &self,
        change: impl FnOnce(&mut FileContents) -> Result<(), String>,
    ) -> Result<Vec<Profile>, String> {
        let path = self.0.lock().map_err(|error| error.to_string())?;
        let mut file = read(&path)?;
        change(&mut file)?;
        let directory = path
            .parent()
            .ok_or("The profiles directory is unavailable.")?;
        fs::create_dir_all(directory).map_err(profile_error)?;
        let mut temporary = tempfile::NamedTempFile::new_in(directory).map_err(profile_error)?;
        serde_json::to_writer_pretty(temporary.as_file_mut(), &file).map_err(profile_error)?;
        temporary.as_file().sync_all().map_err(profile_error)?;
        temporary.persist(path.as_path()).map_err(profile_error)?;
        Ok(file.profiles)
    }
}

fn read(path: &PathBuf) -> Result<FileContents, String> {
    match File::open(path) {
        Ok(file) => serde_json::from_reader(file).map_err(profile_error),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(FileContents::default()),
        Err(error) => Err(profile_error(error)),
    }
}

fn normalized_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err("Profile names must be 1–64 characters long.".into());
    }
    Ok(name.into())
}

fn profile_error(error: impl std::fmt::Display) -> String {
    format!("Profiles file: {error}")
}
