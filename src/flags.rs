use std::{collections::HashMap, process::Command};

pub fn get_build_flags() -> anyhow::Result<HashMap<String, Vec<String>>> {
    let output = Command::new("dpkg-buildflags").output()?;

    if !output.status.success() {
        anyhow::bail!("dpkg-buildflags failed");
    }

    let output = String::from_utf8(output.stdout)?;

    let mut build_flags = HashMap::new();
    for line in output.lines() {
        let (variable, flags) = line.split_once("=").unwrap();
        let flags = flags.split_whitespace().map(|s| s.to_string()).collect();
        build_flags.insert(variable.to_string(), flags);
    }

    Ok(build_flags)
}
