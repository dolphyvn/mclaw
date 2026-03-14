//! Multi-tenant LLM gateway module.
//!
//! This module provides centralized LLM gateway functionality for MClaw.
//! It allows multiple client groups to connect through a single gateway
//! that holds all LLM API keys and routes requests based on client identity.

pub mod auth;
pub mod config;
pub mod server;
