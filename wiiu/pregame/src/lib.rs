#![no_std]

pub mod accounts;
pub mod config;
pub mod launcher;
pub mod session;

#[cfg(horizon_wiiu_cafe)]
mod cafe;
#[cfg(horizon_wiiu_cafe)]
pub mod canvas;
#[cfg(horizon_wiiu_cafe)]
mod mock;

pub const UP: u32 = 1;
pub const DOWN: u32 = 2;
pub const ACCEPT: u32 = 4;
pub const BACK: u32 = 8;
