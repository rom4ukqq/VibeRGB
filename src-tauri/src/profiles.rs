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
    #[serde(default)]
    startup_id: Option<String>,
    profiles: Vec<Profile>,
    #[serde(default)]
    last_scene: Vec<Assignment>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileLibrary {
    profiles: Vec<Profile>,
    startup_id: Option<String>,
}

impl From<FileContents> for ProfileLibrary {
    fn from(file: FileContents) -> Self {
        Self { profiles: file.profiles, startup_id: file.startup_id }
    }
}

pub struct Profiles(Mutex<PathBuf>);

impl Profiles {
    pub fn new(path: PathBuf) -> Self {
        Self(Mutex::new(path))
    }

    pub fn list(&self) -> Result<ProfileLibrary, String> {
        let path = self.0.lock().map_err(|error| error.to_string())?;
        Ok(read(&path)?.into())
    }

    pub fn assignments(&self, id: &str) -> Result<Vec<Assignment>, String> {
        self.list()?.profiles.into_iter().find(|profile| profile.id == id)
            .map(|profile| profile.assignments).ok_or_else(|| "Profile not found.".into())
    }

    pub fn startup(&self) -> Result<Option<Vec<Assignment>>, String> {
        let path = self.0.lock().map_err(|error| error.to_string())?;
        let file = read(&path)?;
        let Some(id) = file.startup_id else {
            return Ok((!file.last_scene.is_empty()).then_some(file.last_scene));
        };
        file.profiles.into_iter().find(|profile| profile.id == id)
            .map(|profile| Some(profile.assignments)).ok_or_else(|| "Profile not found.".into())
    }

    pub fn remember(&self, assignments: Vec<Assignment>) -> Result<(), String> {
        self.edit(|file| {
            file.last_scene = assignments;
            Ok(())
        })
        .map(|_| ())
    }

    pub fn set_startup(&self, id: Option<String>) -> Result<ProfileLibrary, String> {
        self.edit(|file| {
            if id.as_ref().is_some_and(|id| !file.profiles.iter().any(|p| &p.id == id)) {
                return Err("Profile not found.".into());
            }
            file.startup_id = id;
            Ok(())
        })
    }

    pub fn replace(&self, id: &str, assignments: Vec<Assignment>) -> Result<ProfileLibrary, String> {
        if assignments.is_empty() {
            return Err("Apply lighting to at least one zone first.".into());
        }
        self.edit(|file| {
            let profile = file.profiles.iter_mut().find(|profile| profile.id == id)
                .ok_or("Profile not found.")?;
            profile.assignments = assignments;
            Ok(())
        })
    }

    pub fn save(&self, name: String, assignments: Vec<Assignment>) -> Result<ProfileLibrary, String> {
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

    pub fn rename(&self, id: &str, name: String) -> Result<ProfileLibrary, String> {
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

    pub fn delete(&self, id: &str) -> Result<ProfileLibrary, String> {
        self.edit(|file| {
            let index = file
                .profiles
                .iter()
                .position(|profile| profile.id == id)
                .ok_or("Profile not found.")?;
            file.profiles.remove(index);
            if file.startup_id.as_deref() == Some(id) {
                file.startup_id = None;
            }
            Ok(())
        })
    }

    fn edit(
        &self,
        change: impl FnOnce(&mut FileContents) -> Result<(), String>,
    ) -> Result<ProfileLibrary, String> {
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
        Ok(file.into())
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
