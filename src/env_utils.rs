use std::{env, fmt::Display, str::FromStr};

pub fn get_env_var<T: FromStr + Display>(name: &str, default: T) -> T {
    match env::var(name) {
        Ok(val) => val.parse::<T>().unwrap_or_else(|_| {
            eprintln!(
                "invalid '{name}' environment value: {val}, using the default value '{default}'"
            );

            default
        }),
        Err(_) => default,
    }
}

pub fn get_bool_env_var(name: &str, default: bool) -> bool {
    match env::var(name) {
        Ok(val) => match val.to_lowercase().as_str() {
            "1" | "true" | "on" => true,
            "0" | "false" | "off" => false,
            _ => {
                eprintln!("invalid '{name}' environment value: {val}, using the default value '{default}'");

                default
            }
        },
        Err(_) => default,
    }
}
