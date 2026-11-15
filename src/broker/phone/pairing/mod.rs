//! UKEY2 pairing and persistent enrollment, with synthetic interoperability fixtures.

pub mod java_pairing_worker;
pub mod packet;
pub mod paired_socket;
mod pairing_prompt;
mod pairing_prompt_context;
mod pairing_prompt_state;
pub(crate) mod pairing_runtime;
pub mod pairing_server;
mod persistent_enrollment;

#[cfg(test)]
mod java_pairing_worker_tests;
#[cfg(test)]
mod paired_preview;
#[cfg(test)]
mod paired_queue_fixture;
#[cfg(test)]
mod paired_sdk_tests;
#[cfg(test)]
mod paired_socket_tests;
#[cfg(test)]
mod preview_phase;
