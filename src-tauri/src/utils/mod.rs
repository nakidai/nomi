use std::{path::PathBuf, fs::{File, OpenOptions}, io::Error};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct Config {
  pub username: String,
  pub profiles: Vec<Profile>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct Profile {
  id: i32,
  pub version: String,
  pub version_type: String,
  pub path: String,
}

impl Profile {
  pub fn new(
    version: String,
    version_type: String,
    path: String,
    profiles: &Vec<Profile>
  ) -> Self {
    Self {
      id: Self::create_id(&profiles),
      version,
      version_type,
      path,
    }
  }

  pub fn create_id(profiles: &Vec<Profile>) -> i32 {
    let mut max_id: Vec<i32> = vec![];
    for prof in profiles.iter() {
      max_id.push(prof.id)
    }

    match max_id.iter().max() {
      Some(mx) => dbg!(mx + 1),
      None => {
        println!("Vec is empty");
        0
      }
    }
  }
}

impl Config {
  pub fn new(username: String) -> Self {
    Self {
      username,
      profiles: vec![],
    }
  }
  
  pub fn from_file(username: Option<String>) -> Self {
    let conf: (bool, PathBuf) = {
      let config = std::env::current_dir().unwrap().join("config.yaml");
      if config.exists() {
        (true, config)
      } else {
        (false, config)
      }
    };
    match conf.0 {
      true => {
        let f = std::fs::File::open(conf.1).expect("Could not open file");
        let mut read: Config = serde_yaml::from_reader(f).expect("Could not read values");
        match username {
          Some(u) => {
            read.username = u;
            read
          },
          None => read,
        }
      },
      false => {
        Self {
          username: match username {
            Some(u) => u,
            None => panic!(),
        },
          profiles: vec![],
        }
      },
    }
  }

  fn does_exist(&self) -> (bool, PathBuf) {
    let config = std::env::current_dir().unwrap().join("config.yaml");
    if config.exists() {
      return (true, config)
    }
    return (false, config);
  }

  pub fn write_config(&self) -> Result<(), Error> {
    let conf = self.does_exist();
    let mut file: File = OpenOptions::new()
      .read(true)
      .write(true)
      .create(true)
      .append(false)
      .open(conf.1)?;
    if conf.0 { 
      file = std::fs::File::create(self.does_exist().1).unwrap();
    }
    let _ = serde_yaml::to_writer(&mut file, &self);
    println!("created config");
    Ok(())
  }

  pub fn overwrite(&self) {
    let conf = self.does_exist();
    match conf.0 {
      true => {
        let mut file = OpenOptions::new()
          .write(true)
          .truncate(true)
          .open(conf.1)
          .unwrap();

        let _ = serde_yaml::to_writer(&mut file, &self);
      },
      false => {
        self.write_config().unwrap()
      },
    }
  }

  pub fn read_config(&self) -> Result<Config, ()> {
    let conf = self.does_exist();
    if conf.0 {
      let f = std::fs::File::open(conf.1).expect("Could not open file");
      let read: Config = serde_yaml::from_reader(f).expect("Could not read values");
      return Ok(read);
    } else {
      let _ = self.write_config();
      return Err(());
    }
        
  }

  pub fn add_profile(&mut self, profile: Profile) {
    self.profiles.push(profile);
  }

  pub fn get_profile(&self, id: i32) -> Option<&Profile> {
    for prof in self.profiles.iter() {
      if prof.id == id {
        return Some(prof);
      }
    }
    return None;
  }
}